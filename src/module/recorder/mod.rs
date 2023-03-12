pub mod ytarchive;
pub mod ytdlp;

/// The current state of ytarchive.
#[derive(Debug, Clone, TS, Serialize)]
#[ts(export, export_to = "web/src/bindings/")]
pub struct YTStatus {
    version: Option<String>,
    state: YTState,
    last_output: Option<String>,
    last_update: chrono::DateTime<chrono::Utc>,
    video_fragments: Option<u32>,
    audio_fragments: Option<u32>,
    total_size: Option<String>,
    video_quality: Option<String>,
    output_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, TS, Serialize)]
#[ts(export, export_to = "web/src/bindings/")]
pub enum YTState {
    Idle,
    Waiting(Option<DateTime<Utc>>),
    Recording,
    Muxing,
    Finished,
    AlreadyProcessed,
    Ended,
    Interrupted,
    Errored,
}

#[async_trait]
pub trait Recorder<T: Debug + Clone + Sync = Message> {
    async fn record(cfg: Config, task: Task, bus: &mut BusTx<Message>) -> Result<()>
    fn new(config: Arc<RwLock<Config>>) -> Self;
    async fn run(&self, tx: &BusTx<T>, rx: &mut mpsc::Receiver<T>) -> Result<()>;
}
