use quiche::cc_simu;

fn main() {
    env_logger::builder().format_timestamp_nanos().init();

    cc_simu::gui_main().unwrap();
}
