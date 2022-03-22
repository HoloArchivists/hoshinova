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

Copy the `config.example.yaml` to `config.yaml` and edit the file as needed.

```yaml
poll_interval: 60
workdir: /tmp
```

The `poll_interval` is how long (in seconds) to wait before checking the
channel's RSS feed for new videos. There is no known rate limit for the RSS
endpoint so feel free to adjust this parameter.

Workdir is where all temporary files will be stored. Stream fragments,
unfinished downloads, etc.

### ytarchive configuration

```yaml
ytarchive:
  path: ytarchive
  quality: best
  flags:
    - '--vp9'
    - '--thumbnail'
    - '--add-metadata'
    - '--threads'
    - '4'
    - '--output'
    - '%(id)s'
```

By default, the `--wait` and `--merge` flags are added. You can add your own
flags to use cookies, set the number of threads, etc.

### Channel configuration

```yaml
channels:
  - name: Moona ch.
    id: UCP0BspO_AMEe3aQqqpo89Dg
    filters:
      - '(?i)MoonUtau'
      - '(?i)Karaoke'
      - '(?i)Unarchived'
```

The `channels` array contains a list of YouTube channels to monitor. The
`filters` in each `channel` is a list of regular expressions string. If the
title of a stream matches the regex, it will get downloaded. For more
information on Go's regex syntax, run `go doc regexp/syntax`.

In the example above, the `(?i)` marks the expression as case-insensitive.

### Uploader configuration

```yaml
uploaders:
  - name: My Videos folder
    type: local
    config:
      path: /home/kitsune/Videos
      base_url: http://localhost:3000
```

After a live stream is finished downloading, it will get "uploaded" to all of
the uploaders in the list. Right now, there is only the `local` uploader, which
simply moves the resulting video to the destination directory. The `base_url`
parameter will be useful if `path` leads to a webserver root.

In the future, this can be expanded to include other services such as Google
Drive, S3, etc.

### Notifier configuration

```yaml
notifiers:
  - name: My private server
    type: discord
      webhook_url: 'https://discord.com/api/webhooks/...'
```

Notifiers can be registered to send a message when the video has been uploaded.
Right now, only Discord is supported, but more will be added in the future.

## Run

If you used `go install`,

```
~/go/bin/hoshinova
```
