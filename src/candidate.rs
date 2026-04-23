use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub id: CandidateId,
    pub primary: String,
    pub secondary: Option<String>,
    pub searchable: Vec<String>,
    pub action: CandidateAction,
    pub metadata: BTreeMap<String, String>,
}

impl Candidate {
    pub fn new(id: impl Into<CandidateId>, primary: impl Into<String>) -> Self {
        let primary = primary.into();
        Self {
            id: id.into(),
            searchable: vec![primary.clone()],
            primary,
            secondary: None,
            action: CandidateAction::None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_secondary(mut self, secondary: impl Into<String>) -> Self {
        let secondary = secondary.into();
        if !secondary.is_empty() {
            self.searchable.push(secondary.clone());
            self.secondary = Some(secondary);
        }
        self
    }

    pub fn with_secondary_display_only(mut self, secondary: impl Into<String>) -> Self {
        let secondary = secondary.into();
        if !secondary.is_empty() {
            self.secondary = Some(secondary);
        }
        self
    }

    pub fn with_action(mut self, action: CandidateAction) -> Self {
        self.action = action;
        self
    }

    pub fn with_searchable(mut self, field: impl Into<String>) -> Self {
        let field = field.into();
        if !field.is_empty() {
            self.searchable.push(field);
        }
        self
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CandidateId(String);

impl CandidateId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CandidateId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for CandidateId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for CandidateId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidateAction {
    None,
    Exec(Vec<String>),
    DesktopExec(String),
}
