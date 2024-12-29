use time::{macros::format_description, UtcOffset};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{self, fmt::time::OffsetTime, EnvFilter};

pub fn init_tracing() {
    let local_time = OffsetTime::new(
        UtcOffset::from_hms(8, 0, 0).unwrap(),
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
    );

    tracing_subscriber::fmt()
        .with_timer(local_time)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_line_number(true)
        .with_file(true)
        .init();
}
