#[derive(Clone, Debug, PartialEq)]
pub struct InternalSynthesisRequest {
    pub text: String,
    pub voice_id: Option<String>,
    pub model_id: Option<String>,
    pub speed: f32,
    pub output_format: Option<String>,
    pub streaming: bool,
}
