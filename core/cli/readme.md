# lightning-cli

```bash
$ lightning-cli -h
Usage: lightning-cli [OPTIONS] <COMMAND>

Commands:
  run           Start the node
  key           Handle key
  print-config  Print the loaded configuration
  help          Print this message or the help of the given subcommand(s)

Options:
  -c, --config <CONFIG>      Path to the toml configuration file [default: $HOME/.lightning/config.toml]
      --with-mock-consensus  Determines that we should be using the mock consensus backend
  -v...                      Increases the level of verbosity (the max level is -vvv)
      --log-location         Print code location on console logs
  -h, --help                 Print help
  -V, --version              Print version
```
