#[cfg(test)]
mod init_tests {
    use std::{fs, str};

    use assert_cmd::Command;
    use tempdir::TempDir;

    #[test]
    fn missing_network_and_dev() {
        let temp_dir = TempDir::new("test").unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(str::from_utf8(&output.stderr).unwrap().contains(
            "Error: Either --network or --dev must be provided. See --help for details."
        ));

        let config_path = temp_dir.path().join("config.toml");
        assert!(!config_path.exists());
    }

    #[test]
    fn empty_network() {
        let temp_dir = TempDir::new("test").unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init").arg("--network");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(str::from_utf8(&output.stderr).unwrap().contains(
            "error: a value is required for '--network <NETWORK>' but none was supplied"
        ));

        let config_path = temp_dir.path().join("config.toml");
        assert!(!config_path.exists());
    }

    #[test]
    fn with_network_no_args() {
        let temp_dir = TempDir::new("test").unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init").arg("--network").arg("localnet-example");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );

        let config_path = temp_dir.path().join("config.toml");
        assert!(str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(config_path.exists() && fs::metadata(&config_path).map_or(false, |m| m.len() > 0));

        let config = fs::read_to_string(&config_path).unwrap();
        assert!(!config.contains("[application.dev]"));
        assert!(!config.contains("update_epoch_start_to_now = true"));

        assert!(
            str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Generated node key:")
        );
        assert!(
            str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Generated consensus key:")
        );
        assert!(
            str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Genesis block loaded into application state.")
        );
    }

    #[test]
    fn with_network_and_no_generate_keys() {
        let temp_dir = TempDir::new("test").unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init")
            .arg("--network")
            .arg("localnet-example")
            .arg("--no-generate-keys");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );

        let config_path = temp_dir.path().join("config.toml");
        assert!(str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(config_path.exists() && fs::metadata(config_path).map_or(false, |m| m.len() > 0));

        assert!(
            !str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Generated node key:")
        );
        assert!(
            !str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Generated consensus key:")
        );
        // Genesis isn't applied when keys aren't generated.
        assert!(
            !str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Genesis block loaded into application state.")
        );
    }

    #[test]
    fn with_network_and_no_generate_keys_no_apply_genesis() {
        let temp_dir = TempDir::new("test").unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init")
            .arg("--network")
            .arg("localnet-example")
            .arg("--no-generate-keys")
            .arg("--no-apply-genesis");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );

        let config_path = temp_dir.path().join("config.toml");
        assert!(str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(config_path.exists() && fs::metadata(config_path).map_or(false, |m| m.len() > 0));

        assert!(
            !str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Generated node key:")
        );
        assert!(
            !str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Generated consensus key:")
        );
        assert!(
            !str::from_utf8(&output.stdout)
                .unwrap()
                .contains("Genesis block loaded into application state.")
        );
    }

    #[test]
    fn with_dev_and_no_network() {
        let temp_dir = TempDir::new("test").unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init").arg("--dev");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );

        let config_path = temp_dir.path().join("config.toml");
        assert!(str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(config_path.exists());

        let config = fs::read_to_string(&config_path).unwrap();
        assert!(config.contains("network = \"localnet-example\""));
        assert!(config.contains("[application.dev]"));
        assert!(config.contains("update_epoch_start_to_now = true"));
    }

    #[test]
    fn with_dev_and_network() {
        let temp_dir = TempDir::new("test").unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init")
            .arg("--network")
            .arg("localnet-example")
            .arg("--dev");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            str::from_utf8(&output.stderr).unwrap()
        );

        let config_path = temp_dir.path().join("config.toml");
        assert!(str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(config_path.exists());

        let config = fs::read_to_string(&config_path).unwrap();
        assert!(config.contains("network = \"localnet-example\""));
        assert!(config.contains("[application.dev]"));
        assert!(config.contains("update_epoch_start_to_now = true"));
    }
}
