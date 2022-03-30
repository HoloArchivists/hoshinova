# hoshinova

> Monitor YouTube channels and automatically run
> [ytarchive](https://github.com/Kethsar/ytarchive) when the channel goes live.

![Screenshot](https://user-images.githubusercontent.com/7418049/158234855-255f8897-f8a6-40f1-a890-af34336e65b6.png)

**⚠️ Unstable Software**: This program is under heavy development. It works, but
will still undergo a lot of breaking changes. Upgrade with caution.

## Install

Make sure you have [ytarchive](https://github.com/Kethsar/ytarchive) installed
and executable in your PATH.

```
go install github.com/HoloArchivists/hoshinova@main
```

You should now have an executable `~/go/bin/hoshinova`.

## Configure

Check the `config.example.yaml` file for detailed explanations on how to
configure hoshinova.

## Run

If you used `go install`,

```
~/go/bin/hoshinova
```
