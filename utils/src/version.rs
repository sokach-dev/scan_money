pub fn get_version() -> &'static str {
    let v = format!(
        "{}+{}+{}+{}",
        env!("VERGEN_BUILD_DATE"),
        env!("VERGEN_GIT_BRANCH"),
        env!("VERGEN_GIT_SHA"),
        env!("VERGEN_RUSTC_HOST_TRIPLE"),
    );
    Box::leak(v.into_boxed_str())
}
