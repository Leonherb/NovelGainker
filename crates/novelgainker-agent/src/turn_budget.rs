use crate::action_contracts::{AgentActionErrorCodeV1, ProjectRelativePathV1, ToolRequestV1};
use crate::contracts::TurnBudgetV1;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TurnBudgetUsageV1 {
    pub agent_steps: u16,
    pub tool_calls: u16,
    pub unique_project_files: u16,
    pub modified_project_files: u16,
    pub write_transactions: u16,
    pub largest_provider_request_bytes: u32,
}

#[derive(Debug, Clone)]
pub struct TurnBudgetTrackerV1 {
    limits: TurnBudgetV1,
    usage: TurnBudgetUsageV1,
    unique_files: BTreeSet<ProjectRelativePathV1>,
    modified_files: BTreeSet<ProjectRelativePathV1>,
}

impl TurnBudgetTrackerV1 {
    pub fn new(limits: TurnBudgetV1) -> Self {
        Self {
            limits,
            usage: TurnBudgetUsageV1::default(),
            unique_files: BTreeSet::new(),
            modified_files: BTreeSet::new(),
        }
    }

    pub fn usage(&self) -> TurnBudgetUsageV1 {
        self.usage
    }

    pub fn record_agent_step(&mut self) -> Result<(), AgentActionErrorCodeV1> {
        if self.usage.agent_steps >= self.limits.max_agent_steps {
            return Err(AgentActionErrorCodeV1::AgentStepBudgetExceeded);
        }
        self.usage.agent_steps += 1;
        Ok(())
    }

    pub fn record_provider_request(
        &mut self,
        request_bytes: u32,
    ) -> Result<(), AgentActionErrorCodeV1> {
        if request_bytes > self.limits.max_provider_request_bytes {
            return Err(AgentActionErrorCodeV1::ProviderRequestBudgetExceeded);
        }
        self.usage.largest_provider_request_bytes =
            self.usage.largest_provider_request_bytes.max(request_bytes);
        Ok(())
    }

    pub fn record_tool_batch(
        &mut self,
        requests: &[ToolRequestV1],
    ) -> Result<(), AgentActionErrorCodeV1> {
        if requests.is_empty() {
            return Err(AgentActionErrorCodeV1::EmptyToolBatch);
        }
        if requests.len() > self.limits.max_tool_calls_per_batch as usize {
            return Err(AgentActionErrorCodeV1::ToolBatchLimitExceeded);
        }
        let requested = u16::try_from(requests.len())
            .map_err(|_| AgentActionErrorCodeV1::ToolCallBudgetExceeded)?;
        if self.usage.tool_calls.saturating_add(requested) > self.limits.max_tool_calls {
            return Err(AgentActionErrorCodeV1::ToolCallBudgetExceeded);
        }
        if requests
            .iter()
            .enumerate()
            .any(|(ordinal, request)| request.serial_ordinal != ordinal as u16)
        {
            return Err(AgentActionErrorCodeV1::InvalidPayload);
        }
        self.usage.tool_calls += requested;
        Ok(())
    }

    pub fn record_project_file(
        &mut self,
        path: ProjectRelativePathV1,
        modified: bool,
    ) -> Result<(), AgentActionErrorCodeV1> {
        let mut unique = self.unique_files.clone();
        let mut modified_paths = self.modified_files.clone();
        unique.insert(path.clone());
        if modified {
            modified_paths.insert(path);
        }
        if unique.len() > self.limits.max_unique_project_files as usize {
            return Err(AgentActionErrorCodeV1::ProjectFileBudgetExceeded);
        }
        if modified_paths.len() > self.limits.max_modified_project_files as usize {
            return Err(AgentActionErrorCodeV1::ModifiedFileBudgetExceeded);
        }
        self.unique_files = unique;
        self.modified_files = modified_paths;
        self.usage.unique_project_files = self.unique_files.len() as u16;
        self.usage.modified_project_files = self.modified_files.len() as u16;
        Ok(())
    }

    pub fn begin_write_transaction(&mut self) -> Result<(), AgentActionErrorCodeV1> {
        if self.usage.write_transactions >= self.limits.max_write_transactions {
            return Err(AgentActionErrorCodeV1::WriteTransactionBudgetExceeded);
        }
        self.usage.write_transactions += 1;
        Ok(())
    }
}
