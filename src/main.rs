fn main() -> iced::Result {
    // O GUI nunca roda como root: a escrita em /etc/hosts é do helper (ADR-0002).
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("rust-hosts: refusing to run as root; run it as your user");
        std::process::exit(1);
    }

    rust_hosts::app::run()
}
