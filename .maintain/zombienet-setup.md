# 🧟 Zombienet Setup Guide

You can find linux and macOS executables of the Zombienet CLI here:

https://github.com/paritytech/zombienet/releases
Download the Zombienet CLI according to your operating system.

Include the binary on the root of the project and allow it to be executed:

```sh
# On Linux
chmod +x zombienet-linux-x64
# Or on Mac
chmod +x zombienet-macos
```

Tip: If you want the executable to be available system-wide then you can follow these steps (otherwise just download the executable to your working directory):

```sh
wget https://github.com/paritytech/zombienet/releases/download/v1.3.91/zombienet-macos
chmod +x zombienet-macos
cp zombienet-macos /usr/local/bin
```

Make sure Zombienet CLI is installed correctly:

```sh
./zombienet-macos --help
```

You should see some similar output:

```sh
Usage: zombienet [options] [command]

Options:
  -c, --spawn-concurrency <concurrency>  Number of concurrent spawning process to launch, default is 1
  -p, --provider <provider>              Override provider to use (choices: "podman", "kubernetes", "native")
  -m, --monitor                          Start as monitor, do not auto cleanup network
  -h, --help                             display help for command

Commands:
  spawn <networkConfig> [creds]          Spawn the network defined in the config
  test <testFile> [runningNetworkSpec]   Run tests on the network defined
  setup <binaries...>                    Setup is meant for downloading and making dev environment of Zombienet ready
  version                                Prints zombienet version
  help [command]                         display help for command

```
