pub struct AnalysisSummary {}

impl AnalysisSummary {
    pub fn new() -> Self {
        AnalysisSummary {}
    }
}

pub struct AnalysisContext {}

impl AnalysisContext {
    pub fn new() -> Self {
        AnalysisContext {}
    }

    pub fn generate_summary(&self) -> AnalysisSummary {
        AnalysisSummary::new()
    }
}
