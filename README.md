# Story Teller

Story Teller is a Discord bot to play interactive stories created using [twine](https://twinery.org/).
It only supports rudimentary stories, meaning no scripts or engine as of today.

## Usage

You will need a discord token. The bot will look in two locations for a discord token:
* As the environment variable `DISCORD_TOKEN`.
* As the key `DISCORD_TOKEN` in an optional `config.toml` file in the current folder.
You will probably want to prefer the first option, as it's also easy to use secrets for some services like [fly.io](fly.io)
