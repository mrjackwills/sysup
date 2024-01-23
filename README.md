<p align="center">
    <img src='./.github/logo.svg' width='100px' />
    <br>
    <h1 align="center">sysup</h1>
    <div align="center">Send push notifications when a system is powered on</div>
</p>


- [Download \& install](#download--install)
- [Required Envs](#required-envs)
- [Run](#run)
- [Build step](#build-step)
- [Tests](#tests)

## Download & install

### Pre-Built
See the <a href="https://github.com/mrjackwills/sysup/releases/latest" target='_blank' rel='noopener noreferrer'>pre-built binaries</a>

Automatic platform selection & download

*One should always verify <a href='https://github.com/mrjackwills/sysup/blob/main/download.sh' target='_blank' rel='noopener noreferrer'>script content</a> before running in a shell*

```shell
curl https://raw.githubusercontent.com/mrjackwills/sysup/main/download.sh | bash
```

## Required Envs

 Envs that are used by `sysup`
| name           | description                        | required |
| -------------- | ---------------------------------- | :------: |
| `MACHINE_NAME` | Unique name of machine             | ✓        |
| `TOKEN_APP`    | Pushover api app token             | ✓        |
| `TOKEN_USER`   | Pushover api user token            | ✓        |
| `LOG_DEBUG`    | Boolean to toggle debug level logs | ❌       |
| `LOG_TRACE`    | Boolean to toggle trace level logs | ❌       |
| `TIMEZONE`     | Valid timezone of machine          | ❌       |


## Run

Install service

```shell
sudo sysup --install
```

Uninstall service

```shell
sudo sysup --uninstall
```

## Build step

### x86_64

```shell
cargo build --release
```

### Cross platform builds

requires docker & <a href='https://github.com/cross-rs/cross' target='_blank' rel='noopener noreferrer'>cross-rs</a>

#### 64bit pi (pi 4, pi zero w 2)

```shell
cross build --target aarch64-unknown-linux-gnu --release
```

#### 32bit pi (pi zero w)

```shell
cross build --target arm-unknown-linux-musleabihf --release
```

#### Windows

```shell
cross build --target x86_64-pc-windows-gnu --release
```

### Untested on other platforms

## Tests

```shell
cargo test
```
