#!/usr/bin/env bash
# Inserts minimum required entities into the dev database so an event can be sent.
# Usage: ./scripts/seed_dev.sh [--dry-run]
#
# Dependencies: psql, curl, jq
# Requires a running Postgres reachable via DATABASE_URL (sourced from .env).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# ---------------------------------------------------------------------------
# Load .env
# ---------------------------------------------------------------------------
ENV_FILE="$ROOT_DIR/.env"
if [[ -f "$ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$ENV_FILE"
    set +a
else
    echo "ERROR: $ENV_FILE not found. Set DATABASE_URL manually." >&2
    exit 1
fi

: "${DATABASE_URL:?DATABASE_URL is required}"
: "${SERVER_PORT:=3000}"
: "${SERVER_HOST:=localhost}"

BASE_URL="http://${SERVER_HOST}:${SERVER_PORT}"

# ---------------------------------------------------------------------------
# Fixed public IDs (idempotency: re-running the script is safe)
# ---------------------------------------------------------------------------
# Lengths must match VARCHAR constraints in the DB:
#   organizations.public_id VARCHAR(24) → "org_" + 20 chars
#   tenants.public_id       VARCHAR(24) → "ten_" + 20 chars
#   applications.public_id  VARCHAR(20) → "app_" + 16 chars
#   event_types.public_id   VARCHAR(20) → "evt_" + 16 chars
#   endpoints.public_id     VARCHAR(20) → "ep_"  + 16 chars (= 19, fits in 20)
#   endpoint_secrets.public_id VARCHAR(20) → "sec_" + 16 chars

ORG_PUB="org_DevSeed000000000000"   # 4 + 20 = 24
TEN_PUB="ten_DevSeed000000000001"   # 4 + 20 = 24
APP_PUB="app_DevSeed000000001"      # 4 + 16 = 20
EVT_PUB="evt_DevSeed000000001"      # 4 + 16 = 20
EP_PUB="ep_DevSeed000000001"        # 3 + 16 = 19 (fits in VARCHAR(20))
SEC_PUB="sec_DevSeed000000001"      # 4 + 16 = 20

WEBHOOK_URL="https://webhook.site/00000000-0000-0000-0000-000000000000"

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
    echo "=== DRY RUN — SQL will be printed but not executed ==="
fi

# ---------------------------------------------------------------------------
# Build the SQL
# ---------------------------------------------------------------------------
SQL=$(cat <<SQL
DO \$\$
DECLARE
    v_sys_user   UUID := gen_random_uuid();
    v_req_id     UUID := gen_random_uuid();
    v_org_id     UUID;
    v_ten_id     UUID;
    v_app_id     UUID;
    v_ep_id      UUID;
BEGIN
    -- ----------------------------------------------------------------
    -- 1. Organization
    -- ----------------------------------------------------------------
    INSERT INTO organizations (
        id, public_id, name, slug, billing_email,
        created_by, updated_by, request_id
    )
    VALUES (
        gen_random_uuid(),
        '$ORG_PUB',
        'Dev Seed Organization',
        'dev-seed-org',
        'dev@seed.local',
        v_sys_user, v_sys_user, v_req_id
    )
    ON CONFLICT (public_id) DO NOTHING;

    SELECT id INTO v_org_id
    FROM organizations WHERE public_id = '$ORG_PUB';

    RAISE NOTICE '[1/5] organization  public_id=% internal_id=%', '$ORG_PUB', v_org_id;

    -- ----------------------------------------------------------------
    -- 2. Tenant
    -- ----------------------------------------------------------------
    INSERT INTO tenants (
        id, public_id, organization_id, name,
        created_by, updated_by, request_id
    )
    VALUES (
        gen_random_uuid(),
        '$TEN_PUB',
        v_org_id,
        'Dev Seed Tenant',
        v_sys_user, v_sys_user, v_req_id
    )
    ON CONFLICT (public_id) DO NOTHING;

    SELECT id INTO v_ten_id
    FROM tenants WHERE public_id = '$TEN_PUB';

    RAISE NOTICE '[2/5] tenant        public_id=% internal_id=%', '$TEN_PUB', v_ten_id;

    -- ----------------------------------------------------------------
    -- 3. Application
    -- ----------------------------------------------------------------
    INSERT INTO applications (
        id, public_id, organization_id, tenant_id,
        name, description,
        created_by, updated_by, request_id
    )
    VALUES (
        gen_random_uuid(),
        '$APP_PUB',
        v_org_id, v_ten_id,
        'Dev Seed Application',
        'Created by seed_dev.sh',
        v_sys_user, v_sys_user, v_req_id
    )
    ON CONFLICT (public_id) DO NOTHING;

    SELECT id INTO v_app_id
    FROM applications WHERE public_id = '$APP_PUB';

    RAISE NOTICE '[3/5] application   public_id=% internal_id=%', '$APP_PUB', v_app_id;

    -- ----------------------------------------------------------------
    -- 4. Event Type  (minimal schema: object with a single string field)
    -- ----------------------------------------------------------------
    INSERT INTO event_types (
        id, public_id, organization_id, tenant_id,
        name, schema_version, description, event_schema,
        created_by, updated_by, request_id
    )
    VALUES (
        gen_random_uuid(),
        '$EVT_PUB',
        v_org_id, v_ten_id,
        'seed.event.created',
        '1.0',
        'Seed event type',
        '{"type":"object","properties":{"message":{"type":"string"}},"required":[]}'::jsonb,
        v_sys_user, v_sys_user, v_req_id
    )
    ON CONFLICT (public_id) DO NOTHING;

    RAISE NOTICE '[4/5] event_type    public_id=%', '$EVT_PUB';

    -- ----------------------------------------------------------------
    -- 5. Endpoint + Secret
    --    event_types column stores the subscribed event_type public_ids.
    -- ----------------------------------------------------------------
    INSERT INTO endpoints (
        id, public_id, application_id, tenant_id, organization_id,
        endpoint_type, config, event_types,
        created_by, updated_by, request_id
    )
    VALUES (
        gen_random_uuid(),
        '$EP_PUB',
        v_app_id, v_ten_id, v_org_id,
        'http',
        '{"url":"$WEBHOOK_URL","method":"POST","headers":{}}'::jsonb,
        ARRAY['$EVT_PUB'],
        v_sys_user, v_sys_user, v_req_id
    )
    ON CONFLICT (public_id) DO NOTHING;

    SELECT id INTO v_ep_id
    FROM endpoints WHERE public_id = '$EP_PUB';

    -- Placeholder signing secret (not a valid AES-GCM envelope — replace for real delivery testing).
    INSERT INTO endpoint_secrets (
        id, public_id, endpoint_id, tenant_id, organization_id,
        secret, request_id, created_by
    )
    VALUES (
        gen_random_uuid(),
        '$SEC_PUB',
        v_ep_id, v_ten_id, v_org_id,
        'v1\$seed_placeholder_nonce\$seed_placeholder_ciphertext',
        v_req_id, v_sys_user
    )
    ON CONFLICT (public_id) DO NOTHING;

    RAISE NOTICE '[5/5] endpoint      public_id=% internal_id=%', '$EP_PUB', v_ep_id;
    RAISE NOTICE '      secret        public_id=%', '$SEC_PUB';

    RAISE NOTICE '';
    RAISE NOTICE '=== Seed complete ===';
    RAISE NOTICE '  ORG_PUBLIC_ID:       %', '$ORG_PUB';
    RAISE NOTICE '  TENANT_PUBLIC_ID:    %', '$TEN_PUB';
    RAISE NOTICE '  APP_PUBLIC_ID:       %', '$APP_PUB';
    RAISE NOTICE '  EVENT_TYPE_ID:       %', '$EVT_PUB';
    RAISE NOTICE '  ENDPOINT_PUBLIC_ID:  %', '$EP_PUB';
END \$\$;
SQL
)

# ---------------------------------------------------------------------------
# Execute or print
# ---------------------------------------------------------------------------
if [[ "$DRY_RUN" == true ]]; then
    echo "$SQL"
    exit 0
fi

echo "Connecting to Postgres..."
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<< "$SQL"

# ---------------------------------------------------------------------------
# Send a test event via the running API
# ---------------------------------------------------------------------------
echo ""
echo "=== Sending a test event via the API ==="
echo ""

EVENT_PAYLOAD=$(cat <<JSON
{
  "application_id": "$APP_PUB",
  "event_type_id":  "$EVT_PUB",
  "endpoint_id":    "$EP_PUB",
  "payload": { "message": "hello from seed_dev.sh" },
  "tags": { "source": "seed" }
}
JSON
)

echo "POST $BASE_URL/api/v1/events"
echo "$EVENT_PAYLOAD"
echo ""

HTTP_STATUS=$(curl -s -o /tmp/hookly_seed_response.json -w "%{http_code}" \
    -X POST "$BASE_URL/api/v1/events" \
    -H "Content-Type: application/json" \
    -d "$EVENT_PAYLOAD")

echo "HTTP $HTTP_STATUS"
if command -v jq &>/dev/null; then
    jq . /tmp/hookly_seed_response.json
else
    cat /tmp/hookly_seed_response.json
fi

if [[ "$HTTP_STATUS" == "201" || "$HTTP_STATUS" == "200" ]]; then
    echo ""
    echo "Event sent successfully."
else
    echo ""
    echo "WARNING: Event request returned HTTP $HTTP_STATUS. Is the server running on $BASE_URL?" >&2
fi
