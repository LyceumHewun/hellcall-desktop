// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(not(debug_assertions))]
    std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--disable-features=msWebOOUI,msPdfOOUI,msEdgeTranslate,msSmartScreen,msEdgeHistoryImport,msEdgeCollections,msEdgeShopping,msEdgeSidebar,msEdgeWritingAssist,msEdgeHub \
             --disable-extensions \
             --disable-component-update \
             --disable-background-networking \
             --disable-sync \
             --metrics-recording-only \
             --no-sandbox \
             --no-first-run \
             --no-default-browser-check \
             --disable-logging"
        );
    hellcall_desktop_lib::run()
}
