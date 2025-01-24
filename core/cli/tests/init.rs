#[cfg(test)]
mod init_tests {
    use std::{fs, str};

    use assert_cmd::Command;
    use tempfile::tempdir;

    #[test]
    fn missing_network_and_dev() {
        let temp_dir = tempdir().unwrap();
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
        let temp_dir = tempdir().unwrap();
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
        let temp_dir = tempdir().unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init").arg("--network").arg("localnet-example");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            "{}\n{}",
            str::from_utf8(&output.stderr).unwrap(),
            str::from_utf8(&output.stdout).unwrap()
        );
        assert!(
            output.stderr.is_empty(),
            "{}\n{}",
            str::from_utf8(&output.stderr).unwrap(),
            str::from_utf8(&output.stdout).unwrap()
        );

        let config_path = temp_dir.path().join("config.toml");
        assert!(str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Node configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(config_path.exists() && fs::metadata(&config_path).is_ok_and(|m| m.len() > 0));
        let config = fs::read_to_string(&config_path).unwrap();
        assert!(!config.contains("[application.dev]"));
        assert!(!config.contains("update_epoch_start_to_now = true"));

        let genesis_path = temp_dir.path().join("genesis.toml");
        assert!(!str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Genesis configuration written to {}",
            genesis_path.to_string_lossy()
        )));
        assert!(!genesis_path.exists());

        assert!(str::from_utf8(&output.stdout)
            .unwrap()
            .contains("Generated node key:"));
        assert!(str::from_utf8(&output.stdout)
            .unwrap()
            .contains("Generated consensus key:"));
    }

    #[test]
    fn with_network_and_no_generate_keys() {
        let temp_dir = tempdir().unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init")
            .arg("--network")
            .arg("localnet-example")
            .arg("--no-generate-keys");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            "{}\n{}",
            str::from_utf8(&output.stderr).unwrap(),
            str::from_utf8(&output.stdout).unwrap()
        );
        assert!(
            output.stderr.is_empty(),
            "{}\n{}",
            str::from_utf8(&output.stderr).unwrap(),
            str::from_utf8(&output.stdout).unwrap()
        );

        let config_path = temp_dir.path().join("config.toml");
        assert!(str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Node configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(config_path.exists() && fs::metadata(config_path).is_ok_and(|m| m.len() > 0));

        let genesis_path = temp_dir.path().join("genesis.toml");
        assert!(!str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Genesis configuration written to {}",
            genesis_path.to_string_lossy()
        )));
        assert!(!genesis_path.exists());

        assert!(!str::from_utf8(&output.stdout)
            .unwrap()
            .contains("Generated node key:"));
        assert!(!str::from_utf8(&output.stdout)
            .unwrap()
            .contains("Generated consensus key:"));
    }

    #[test]
    fn with_dev_and_no_network() {
        let temp_dir = tempdir().unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init").arg("--dev");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            "{}\n{}",
            str::from_utf8(&output.stderr).unwrap(),
            str::from_utf8(&output.stdout).unwrap()
        );
        assert!(
            output.stderr.is_empty(),
            "{}\n{}",
            str::from_utf8(&output.stderr).unwrap(),
            str::from_utf8(&output.stdout).unwrap()
        );

        println!("{}", str::from_utf8(&output.stdout).unwrap());

        let config_path = temp_dir.path().join("config.toml");
        assert!(str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Node configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(config_path.exists());

        let genesis_path = temp_dir.path().join("genesis.toml");
        assert!(str::from_utf8(&output.stdout)
            .unwrap()
            .contains(&format!(" written to {}", genesis_path.to_string_lossy())));
        assert!(genesis_path.exists());

        let config = fs::read_to_string(&config_path).unwrap();
        assert!(config
            .contains(format!("genesis_path = \"{}\"", genesis_path.to_string_lossy()).as_str()));
        assert!(config.contains("[application.dev]"));
        assert!(config.contains("update_epoch_start_to_now = true"));
    }

    #[test]
    fn with_dev_and_network() {
        let temp_dir = tempdir().unwrap();
        let mut cmd = Command::cargo_bin("lightning-node").unwrap();
        cmd.arg("init")
            .arg("--network")
            .arg("localnet-example")
            .arg("--dev");
        cmd.env("LIGHTNING_HOME", temp_dir.path());

        let output = cmd.output().unwrap();

        assert!(!output.status.success());

        assert!(str::from_utf8(&output.stderr)
            .unwrap()
            .contains("Cannot specify both --dev and --network"));

        let config_path = temp_dir.path().join("config.toml");
        assert!(!str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Node configuration file written to {}",
            config_path.to_string_lossy()
        )));
        assert!(!config_path.exists());

        let genesis_path = temp_dir.path().join("genesis.toml");
        assert!(!str::from_utf8(&output.stdout).unwrap().contains(&format!(
            "Genesis configuration written to {}",
            genesis_path.to_string_lossy()
        )));
        assert!(!genesis_path.exists());
    }
}
