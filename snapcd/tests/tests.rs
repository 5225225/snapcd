#[test]
fn init_test() {
    use assert_cmd::Command;

    let dir = assert_fs::TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("snapcd").unwrap();

    let assert = cmd.arg("init").current_dir(dir.path()).assert();

    assert.success();
}

#[test]
fn commit_test() {
    use assert_cmd::Command;
    use assert_fs::fixture::{FileWriteStr, PathChild};

    let dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("snapcd")
        .unwrap()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success();

    dir.child("a").write_str("a").unwrap();
    dir.child("b").write_str("old").unwrap();

    Command::cargo_bin("snapcd")
        .unwrap()
        .arg("commit")
        .arg("-m")
        .arg("0")
        .current_dir(dir.path())
        .assert()
        .success();

    Command::cargo_bin("snapcd")
        .unwrap()
        .arg("status")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(indoc::indoc!("HEAD: main [borfq54p]\n"));

    std::fs::remove_file(dir.child("a").path()).unwrap();
    dir.child("b").write_str("new").unwrap();
    dir.child("c").write_str("c").unwrap();

    Command::cargo_bin("snapcd")
        .unwrap()
        .arg("commit")
        .arg("-m")
        .arg("1")
        .current_dir(dir.path())
        .assert()
        .success();

    Command::cargo_bin("snapcd")
        .unwrap()
        .arg("status")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(indoc::indoc!("HEAD: main [brg4j5iz]\n"));

    Command::cargo_bin("snapcd")
        .unwrap()
        .arg("diff")
        .arg("--from")
        .arg("borfq54p")
        .arg("--to")
        .arg("brg4j5iz")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(indoc::indoc!(
            "
        added:
          a
        deleted:
          c
        modified:
          b
        "
        ));
}

#[test]
fn extract_test() {
    use assert_cmd::Command;
    use assert_fs::fixture::{FileWriteStr, PathChild};

    let dir = assert_fs::TempDir::new().unwrap();

    let assert = Command::cargo_bin("snapcd")
        .unwrap()
        .arg("init")
        .current_dir(dir.path())
        .assert();

    assert.success();

    dir.child("dir")
        .child("file")
        .write_str("contents")
        .unwrap();

    let assert = Command::cargo_bin("snapcd")
        .unwrap()
        .arg("commit")
        .arg("-m")
        .arg("0")
        .current_dir(dir.path())
        .assert();

    assert.success();

    let assert = Command::cargo_bin("snapcd")
        .unwrap()
        .arg("status")
        .current_dir(dir.path())
        .assert();

    let expected_output = "HEAD: main [bpqm6eue]\n";

    assert.success().stdout(expected_output);

    let to = assert_fs::TempDir::new().unwrap();

    let extract = Command::cargo_bin("snapcd")
        .unwrap()
        .arg("fetch")
        .arg("bdcyh364")
        .arg(to.path())
        .current_dir(dir.path())
        .assert();

    extract.success();
}
