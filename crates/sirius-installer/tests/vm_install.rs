//! End-to-end install test. Ignored by default: needs root, a scratch disk, and a
//! live target environment. Run inside the VM with:
//!   sudo -E cargo test --test vm_install -- --ignored vm_full_install
//!
//! Set SIRIUS_TEST_DISK to a throwaway block device (e.g. /dev/vdb).

use std::process::Command;

#[test]
#[ignore = "requires root + scratch disk + live environment"]
fn vm_full_install() {
    let disk = std::env::var("SIRIUS_TEST_DISK")
        .expect("set SIRIUS_TEST_DISK to a throwaway block device");
    assert!(disk.starts_with("/dev/"), "refusing non-/dev disk: {disk}");

    let request = format!(
        r#"{{"bootc_image":"ghcr.io/example/os:latest","repart_dir":"/usr/share/sirius/repart.d",
            "target_disk":"{disk}","encrypt":false,"tpm":false,"encryption_key":"",
            "locale":"en_US","keyboard":"us","timezone":"UTC","hostname":"localhost",
            "username":"demo","full_name":"Demo"}}"#
    );

    let exe = env!("CARGO_BIN_EXE_sirius");
    let mut child = Command::new(exe)
        .arg("run-playbook")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn runner");
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(request.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Finished"), "install did not finish: {stdout}");
    assert!(out.status.success());
}
