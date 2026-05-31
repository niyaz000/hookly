#!/usr/bin/env bash
# Hookly API — end-to-end workflow
# Covers: org → tenant → user → team → app → event-type → endpoint → schedule (cron) → event
#
# Prerequisites: jq, curl, server running on localhost:3000
# Run the server:  make run   (or cargo run)
#
# Usage: source this file then call steps individually, or run it end-to-end:
#   chmod +x api_examples.sh && ./api_examples.sh

set -euo pipefail

BASE="http://localhost:3000/api/v1"
H='-H "Content-Type: application/json"'

# ─────────────────────────────────────────────────────────────────────────────
# Health check
# ─────────────────────────────────────────────────────────────────────────────

curl -s http://localhost:3000/api/health

# ─────────────────────────────────────────────────────────────────────────────
# 1. ORGANIZATIONS
# ─────────────────────────────────────────────────────────────────────────────

# 1a. Create organization
ORG_RESP=$(curl -s -X POST "$BASE/organizations" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Acme Corp",
    "slug": "acme-corp",
    "billing_email": "billing@acme.com",
    "tags": { "tier": "enterprise" }
  }')
echo "$ORG_RESP" | jq .
ORG_ID=$(echo "$ORG_RESP" | jq -r '.data.public_id')

# 1b. Get organization
curl -s "$BASE/organizations/$ORG_ID" | jq .

# 1c. List organizations
curl -s "$BASE/organizations" | jq .

# 1d. Update organization
curl -s -X PATCH "$BASE/organizations/$ORG_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Acme Corporation",
    "version": 1
  }' | jq .

# 1e. Suspend organization
curl -s -X POST "$BASE/organizations/$ORG_ID/suspend" | jq .

# 1f. Restore organization
curl -s -X POST "$BASE/organizations/$ORG_ID/restore" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 2. TENANTS
# ─────────────────────────────────────────────────────────────────────────────

# 2a. Create tenant
TENANT_RESP=$(curl -s -X POST "$BASE/tenants" \
  -H "Content-Type: application/json" \
  -d "{
    \"organization_id\": \"$ORG_ID\",
    \"name\": \"Acme US Division\",
    \"description\": \"North America operations\",
    \"tags\": { \"region\": \"us-east\" }
  }")
echo "$TENANT_RESP" | jq .
TENANT_ID=$(echo "$TENANT_RESP" | jq -r '.data.public_id')

# 2b. Get tenant
curl -s "$BASE/tenants/$TENANT_ID" | jq .

# 2c. List tenants
curl -s "$BASE/tenants" | jq .

# 2d. Update tenant
curl -s -X PATCH "$BASE/tenants/$TENANT_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Acme US Division (Updated)",
    "version": 1
  }' | jq .

# 2e. Suspend tenant
curl -s -X POST "$BASE/tenants/$TENANT_ID/suspend" | jq .

# 2f. Reactivate tenant
curl -s -X POST "$BASE/tenants/$TENANT_ID/reactivate" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 3. USERS
# ─────────────────────────────────────────────────────────────────────────────

# 3a. Create user
USER_RESP=$(curl -s -X POST "$BASE/users" \
  -H "Content-Type: application/json" \
  -d "{
    \"organization_id\": \"$ORG_ID\",
    \"tenant_id\": \"$TENANT_ID\",
    \"email\": \"alice@acme.com\",
    \"phone\": \"+14155552671\",
    \"metadata\": { \"department\": \"engineering\" }
  }")
echo "$USER_RESP" | jq .
USER_ID=$(echo "$USER_RESP" | jq -r '.data.public_id')

# 3b. Get user
curl -s "$BASE/users/$USER_ID" | jq .

# 3c. List users
curl -s "$BASE/users" | jq .

# 3d. Update user
curl -s -X PATCH "$BASE/users/$USER_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "phone": "+14155559999",
    "version": 1
  }' | jq .

# 3e. Suspend user
curl -s -X POST "$BASE/users/$USER_ID/suspend" | jq .

# 3f. Reactivate user
curl -s -X POST "$BASE/users/$USER_ID/reactivate" | jq .

# 3g. Lock user
curl -s -X POST "$BASE/users/$USER_ID/lock" \
  -H "Content-Type: application/json" \
  -d '{ "reason": "Too many failed login attempts" }' | jq .

# 3h. Unlock user
curl -s -X POST "$BASE/users/$USER_ID/unlock" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 4. TEAMS
# ─────────────────────────────────────────────────────────────────────────────

# 4a. Create team
TEAM_RESP=$(curl -s -X POST "$BASE/teams" \
  -H "Content-Type: application/json" \
  -d "{
    \"organization_id\": \"$ORG_ID\",
    \"tenant_id\": \"$TENANT_ID\",
    \"name\": \"Backend Team\",
    \"description\": \"Owns the core API services\"
  }")
echo "$TEAM_RESP" | jq .
TEAM_ID=$(echo "$TEAM_RESP" | jq -r '.data.public_id')

# 4b. Get team
curl -s "$BASE/teams/$TEAM_ID" | jq .

# 4c. List teams
curl -s "$BASE/teams" | jq .

# 4d. Update team
curl -s -X PATCH "$BASE/teams/$TEAM_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Backend Platform Team",
    "version": 1
  }' | jq .

# 4e. Add team members
curl -s -X PATCH "$BASE/teams/$TEAM_ID/members" \
  -H "Content-Type: application/json" \
  -d "{
    \"user_ids\": [\"$USER_ID\"]
  }" | jq .

# 4f. Remove a team member (replace MEMBER_PUBLIC_ID with the actual value)
# curl -s -X DELETE "$BASE/teams/$TEAM_ID/members/<MEMBER_PUBLIC_ID>" | jq .

# 4g. Delete team
# curl -s -X DELETE "$BASE/teams/$TEAM_ID" | jq .

# 4h. Restore deleted team
# curl -s -X PATCH "$BASE/teams/$TEAM_ID/restore" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 5. APPLICATIONS
# ─────────────────────────────────────────────────────────────────────────────

# 5a. Create application
APP_RESP=$(curl -s -X POST "$BASE/applications" \
  -H "Content-Type: application/json" \
  -d "{
    \"organization_id\": \"$ORG_ID\",
    \"tenant_id\": \"$TENANT_ID\",
    \"name\": \"Acme Payments Service\",
    \"description\": \"Handles payment lifecycle events\",
    \"tags\": { \"env\": \"production\" }
  }")
echo "$APP_RESP" | jq .
APP_ID=$(echo "$APP_RESP" | jq -r '.data.public_id')

# 5b. Get application
curl -s "$BASE/applications/$APP_ID" | jq .

# 5c. Delete application (soft delete)
# curl -s -X DELETE "$BASE/applications/$APP_ID" | jq .

# 5d. Restore application
# curl -s -X POST "$BASE/applications/$APP_ID/restore" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 6. EVENT TYPES
# ─────────────────────────────────────────────────────────────────────────────

# 6a. Create event type with a JSON schema
ET_RESP=$(curl -s -X POST "$BASE/event-types" \
  -H "Content-Type: application/json" \
  -d "{
    \"organization_id\": \"$ORG_ID\",
    \"tenant_id\": \"$TENANT_ID\",
    \"name\": \"payment.completed\",
    \"description\": \"Fired when a payment is successfully processed\",
    \"schema_version\": \"1.0.0\",
    \"event_schema\": {
      \"type\": \"object\",
      \"properties\": {
        \"amount\": { \"type\": \"number\", \"minimum\": 0 },
        \"currency\": { \"type\": \"string\", \"max_length\": 3 },
        \"payment_id\": { \"type\": \"string\" }
      },
      \"required\": [\"amount\", \"currency\", \"payment_id\"]
    }
  }")
echo "$ET_RESP" | jq .
ET_ID=$(echo "$ET_RESP" | jq -r '.data.public_id')

# 6b. Get event type
curl -s "$BASE/event-types/$ET_ID" | jq .

# 6c. Get schema
curl -s "$BASE/event-types/$ET_ID/schema" | jq .

# 6d. List event types
curl -s "$BASE/event-types?tenant_id=$TENANT_ID" | jq .

# 6e. Create a new schema version
curl -s -X POST "$BASE/event-types/$ET_ID/versions" \
  -H "Content-Type: application/json" \
  -d '{
    "schema_version": "1.1.0",
    "description": "Added optional metadata field",
    "event_schema": {
      "type": "object",
      "properties": {
        "amount":     { "type": "number", "minimum": 0 },
        "currency":   { "type": "string", "max_length": 3 },
        "payment_id": { "type": "string" },
        "metadata":   { "type": "object" }
      },
      "required": ["amount", "currency", "payment_id"]
    }
  }' | jq .

# 6f. List versions
curl -s "$BASE/event-types/$ET_ID/versions" | jq .

# 6g. Update description
curl -s -X PATCH "$BASE/event-types/$ET_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Fired when a payment is successfully processed (updated)",
    "version": 1
  }' | jq .

# 6h. Archive event type
curl -s -X POST "$BASE/event-types/$ET_ID/archive" | jq .

# 6i. Unarchive event type
curl -s -X POST "$BASE/event-types/$ET_ID/unarchive" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 7. ENDPOINTS  (webhook receiver URL)
# ─────────────────────────────────────────────────────────────────────────────

# 7a. Create endpoint (HTTP webhook)
EP_RESP=$(curl -s -X POST "$BASE/endpoints" \
  -H "Content-Type: application/json" \
  -d "{
    \"application_id\": \"$APP_ID\",
    \"description\": \"Production webhook receiver\",
    \"endpoint_type\": \"Http\",
    \"config\": {
      \"url\": \"https://webhook.acme.com/hookly/ingest\",
      \"method\": \"POST\",
      \"headers\": { \"X-Source\": \"hookly\" }
    },
    \"event_types\": [\"$ET_ID\"],
    \"rate_limit_per_minute\": 100,
    \"tags\": { \"env\": \"production\" }
  }")
echo "$EP_RESP" | jq .
EP_ID=$(echo "$EP_RESP" | jq -r '.data.public_id')

# 7b. Get endpoint
curl -s "$BASE/endpoints/$EP_ID" | jq .

# 7c. List endpoints
curl -s "$BASE/endpoints" | jq .

# 7d. Update endpoint
curl -s -X PATCH "$BASE/endpoints/$EP_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Production webhook receiver (v2)",
    "rate_limit_per_minute": 200,
    "version": 1
  }' | jq .

# 7e. Get signing secret
curl -s "$BASE/endpoints/$EP_ID/secret" | jq .

# 7f. Rotate signing secret (old secret valid for 300 s)
curl -s -X POST "$BASE/endpoints/$EP_ID/secret/rotate" \
  -H "Content-Type: application/json" \
  -d '{ "expiry_seconds": 300 }' | jq .

# 7g. Pause endpoint
curl -s -X POST "$BASE/endpoints/$EP_ID/pause" | jq .

# 7h. Resume endpoint
curl -s -X POST "$BASE/endpoints/$EP_ID/resume" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 8. SCHEDULES  (cron job — fires event on a timer)
# ─────────────────────────────────────────────────────────────────────────────

# 8a. Create schedule — daily at 09:00 UTC
SCHED_RESP=$(curl -s -X POST "$BASE/schedules" \
  -H "Content-Type: application/json" \
  -d "{
    \"organization_id\": \"$ORG_ID\",
    \"tenant_id\": \"$TENANT_ID\",
    \"name\": \"Daily Payment Summary\",
    \"description\": \"Fires payment.completed event every day at 09:00 UTC\",
    \"event_type_id\": \"$ET_ID\",
    \"endpoint_ids\": [\"$EP_ID\"],
    \"cron_expression\": \"0 9 * * *\",
    \"timezone\": \"UTC\",
    \"payload\": {
      \"amount\": 0,
      \"currency\": \"USD\",
      \"payment_id\": \"scheduled-summary\"
    }
  }")
echo "$SCHED_RESP" | jq .
SCHED_ID=$(echo "$SCHED_RESP" | jq -r '.data.public_id')

# 8b. Get schedule
curl -s "$BASE/schedules/$SCHED_ID" | jq .

# 8c. List schedules
curl -s "$BASE/schedules" | jq .

# 8d. Update schedule — change to hourly
curl -s -X PATCH "$BASE/schedules/$SCHED_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Hourly Payment Summary",
    "cron_expression": "0 * * * *",
    "version": 1
  }' | jq .

# 8e. Manually trigger schedule (fires immediately, returns execution)
EXEC_RESP=$(curl -s -X POST "$BASE/schedules/$SCHED_ID/trigger")
echo "$EXEC_RESP" | jq .
EXEC_ID=$(echo "$EXEC_RESP" | jq -r '.data.public_id')

# 8f. List executions for this schedule
curl -s "$BASE/schedules/$SCHED_ID/executions" | jq .

# 8g. Get a specific execution
curl -s "$BASE/schedules/$SCHED_ID/executions/$EXEC_ID" | jq .

# 8h. Pause schedule
curl -s -X PATCH "$BASE/schedules/$SCHED_ID/pause" | jq .

# 8i. Resume schedule
curl -s -X PATCH "$BASE/schedules/$SCHED_ID/resume" | jq .

# 8j. Delete schedule (soft delete)
curl -s -X DELETE "$BASE/schedules/$SCHED_ID" | jq .

# 8k. Restore schedule
curl -s -X PATCH "$BASE/schedules/$SCHED_ID/restore" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 9. EVENTS  (fire a one-off event)
# ─────────────────────────────────────────────────────────────────────────────

# 9a. Create (publish) an event
EVENT_RESP=$(curl -s -X POST "$BASE/events" \
  -H "Content-Type: application/json" \
  -d "{
    \"application_id\": \"$APP_ID\",
    \"event_type_id\": \"$ET_ID\",
    \"endpoint_id\": \"$EP_ID\",
    \"idempotency_key\": \"pay_abc123\",
    \"payload\": {
      \"amount\": 9900,
      \"currency\": \"USD\",
      \"payment_id\": \"pay_abc123\"
    },
    \"tags\": { \"source\": \"checkout\" }
  }")
echo "$EVENT_RESP" | jq .
EVENT_ID=$(echo "$EVENT_RESP" | jq -r '.data.public_id')

# 9b. Get event by ID
curl -s "$BASE/events/$EVENT_ID" | jq .

# 9c. List events for an application
curl -s "$BASE/events?application_id=$APP_ID&limit=20" | jq .

# 9d. Filter events by event type
curl -s "$BASE/events?application_id=$APP_ID&event_type_id=$ET_ID" | jq .

# 9e. Idempotency — same idempotency_key returns 200 (not 201) with the original event
curl -s -X POST "$BASE/events" \
  -H "Content-Type: application/json" \
  -d "{
    \"application_id\": \"$APP_ID\",
    \"event_type_id\": \"$ET_ID\",
    \"endpoint_id\": \"$EP_ID\",
    \"idempotency_key\": \"pay_abc123\",
    \"payload\": { \"amount\": 9900, \"currency\": \"USD\", \"payment_id\": \"pay_abc123\" }
  }" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# 10. INVITES
# ─────────────────────────────────────────────────────────────────────────────

# 10a. Create invite
INVITE_RESP=$(curl -s -X POST "$BASE/invites" \
  -H "Content-Type: application/json" \
  -d "{
    \"organization_id\": \"$ORG_ID\",
    \"tenant_id\": \"$TENANT_ID\",
    \"user_email\": \"bob@acme.com\",
    \"role\": \"member\",
    \"created_by\": \"$USER_ID\",
    \"expires_at\": \"2026-12-31T23:59:59Z\"
  }")
echo "$INVITE_RESP" | jq .
INVITE_ID=$(echo "$INVITE_RESP" | jq -r '.data.public_id')
INVITE_TOKEN=$(echo "$INVITE_RESP" | jq -r '.data.token // empty')

# 10b. Get invite by ID
curl -s "$BASE/invites/$INVITE_ID" | jq .

# 10c. List invites
curl -s "$BASE/invites?organization_id=$ORG_ID" | jq .

# 10d. Resend invite email
curl -s -X POST "$BASE/invites/$INVITE_ID/resend" | jq .

# 10e. Verify invite token (check it is still valid)
curl -s -X POST "$BASE/invites/verify" \
  -H "Content-Type: application/json" \
  -d "{\"token\": \"$INVITE_TOKEN\"}" | jq .

# 10f. Accept invite (creates the user membership)
curl -s -X POST "$BASE/invites/accept" \
  -H "Content-Type: application/json" \
  -d "{\"token\": \"$INVITE_TOKEN\"}" | jq .

# 10g. Revoke invite
curl -s -X POST "$BASE/invites/$INVITE_ID/revoke" | jq .

# 10h. Delete invite
curl -s -X DELETE "$BASE/invites/$INVITE_ID" | jq .

# ─────────────────────────────────────────────────────────────────────────────
# Error response examples (expected 4xx)
# ─────────────────────────────────────────────────────────────────────────────

# 422 — validation_error: multiple field errors collected at once
curl -s -X POST "$BASE/organizations" \
  -H "Content-Type: application/json" \
  -d '{"name": "", "slug": "BAD SLUG!!"}' | jq .

# 400 — bad_request: malformed JSON body
curl -s -X POST "$BASE/organizations" \
  -H "Content-Type: application/json" \
  -d '{bad json' | jq .

# 400 — bad_request: wrong Content-Type
curl -s -X POST "$BASE/organizations" \
  -H "Content-Type: text/plain" \
  -d '{"name":"x","slug":"x"}' | jq .

# 404 — not_found: unmatched route (fallback handler)
curl -s "$BASE/does-not-exist" | jq .
