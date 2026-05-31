use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{common::types::RequestContext, error::AppError};

use super::{
    models::{
        AddTeamMembersRequest, CreateTeamRequest, ListTeamsQuery, ListTeamsResponse,
        TeamMemberResponse, TeamResponse, UpdateTeamRequest,
    },
    repository::TeamRepository,
};

pub struct TeamService {
    repo: TeamRepository,
}

impl TeamService {
    pub fn new(repo: TeamRepository) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(name = %req.name))]
    pub async fn create(
        &self,
        req: CreateTeamRequest,
        ctx: RequestContext,
    ) -> Result<TeamResponse, AppError> {
        req.validate()?;
        info!("creating team");
        let team = self.repo.create(req, ctx).await?;
        info!(public_id = %team.public_id, "team created");
        Ok(TeamResponse::from(team))
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_public_id(&self, public_id: String) -> Result<TeamResponse, AppError> {
        info!("fetching team");
        self.repo
            .get_by_public_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("team not found");
                AppError::NotFound(format!("Team not found: {public_id}"))
            })
            .map(TeamResponse::from)
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn update(
        &self,
        public_id: String,
        req: UpdateTeamRequest,
        ctx: RequestContext,
    ) -> Result<TeamResponse, AppError> {
        req.validate()?;
        info!("updating team");
        let team = self
            .repo
            .update(&public_id, req, ctx)
            .await?
            .ok_or_else(|| {
                warn!("team not found for update");
                AppError::NotFound(format!("Team not found: {public_id}"))
            })?;
        info!("team updated");
        Ok(TeamResponse::from(team))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete(&self, public_id: String, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting team");
        let deleted = self.repo.delete(&public_id, ctx).await?;
        if !deleted {
            warn!("team not found for delete");
            return Err(AppError::NotFound(format!("Team not found: {public_id}")));
        }
        info!("team deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn restore(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<TeamResponse, AppError> {
        info!("restoring team");
        let team = self.repo.restore(&public_id, ctx).await?.ok_or_else(|| {
            warn!("team not found for restore");
            AppError::NotFound(format!("Team not found: {public_id}"))
        })?;
        info!("team restored");
        Ok(TeamResponse::from(team))
    }

    #[tracing::instrument(skip(self))]
    pub async fn list(&self, query: ListTeamsQuery) -> Result<ListTeamsResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

        let (teams, next_cursor_id) = self
            .repo
            .list(limit, cursor, query.organization_id, query.tenant_id)
            .await?;

        Ok(ListTeamsResponse {
            items: teams.into_iter().map(TeamResponse::from).collect(),
            next_cursor: next_cursor_id.map(encode_cursor),
            limit,
        })
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn add_members(
        &self,
        team_public_id: String,
        req: AddTeamMembersRequest,
        ctx: RequestContext,
    ) -> Result<Vec<TeamMemberResponse>, AppError> {
        req.validate()?;
        info!("adding team members");
        let team = self
            .repo
            .get_by_public_id(&team_public_id)
            .await?
            .ok_or_else(|| {
                warn!("team not found for adding members");
                AppError::NotFound(format!("Team not found: {team_public_id}"))
            })?;
        let members = self.repo.add_members(&team, req.user_ids, ctx).await?;
        info!(count = members.len(), "team members added");
        Ok(members.into_iter().map(TeamMemberResponse::from).collect())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn remove_member(
        &self,
        team_public_id: String,
        member_public_id: String,
        ctx: RequestContext,
    ) -> Result<(), AppError> {
        info!("removing team member");
        let team = self
            .repo
            .get_by_public_id(&team_public_id)
            .await?
            .ok_or_else(|| {
                warn!("team not found for removing member");
                AppError::NotFound(format!("Team not found: {team_public_id}"))
            })?;
        let removed = self
            .repo
            .remove_member(team.id, &member_public_id, ctx)
            .await?;
        if !removed {
            warn!("team member not found");
            return Err(AppError::NotFound(format!(
                "Team member not found: {member_public_id}"
            )));
        }
        info!("team member removed");
        Ok(())
    }
}

fn encode_cursor(id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(id.as_bytes())
}

fn decode_cursor(cursor: &str) -> Result<Uuid, AppError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| AppError::BadRequest("invalid cursor".into()))?;
    Uuid::from_slice(&bytes).map_err(|_| AppError::BadRequest("invalid cursor".into()))
}
