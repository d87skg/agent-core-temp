pub struct IntentPredictor;

impl IntentPredictor {
    pub fn new() -> Self {
        Self
    }

    pub fn risk_score(&self, intent: &str) -> f64 {
        if intent.contains("transfer") || intent.contains("withdraw") || intent.contains("delete") {
            0.95
        } else {
            0.1
        }
    }
}