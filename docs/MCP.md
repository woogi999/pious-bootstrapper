# Connecting AI apps to Pious (MCP)

Pious runs a [Model Context Protocol](https://modelcontextprotocol.io)
server, so AI apps can use it: see your games, accounts and friends, launch
and join games, record clips, run macros, and play in a Roblox window by
looking at it, listening to it and pressing keys, typing and clicking in it.

## The quick way

1. Open **Settings → Plugins → AI apps (MCP)**.
2. Click **Connect** next to your app: Claude Desktop, Claude Code, Cursor
   or Codex. This turns the server on and adds Pious to that app's
   settings.
3. Restart the app if it was open.

Pious keeps a backup of the app's settings file next to it
(`….pious-backup`) before changing it, and only adds or replaces its own
`pious` entry. **Remove** takes it back out.

| App | File Pious edits |
| --- | --- |
| Claude Desktop | `%APPDATA%\Claude\claude_desktop_config.json` |
| Claude Code | `%USERPROFILE%\.claude.json` (for every project) |
| Cursor | `%USERPROFILE%\.cursor\mcp.json` |
| Codex | `%USERPROFILE%\.codex\config.toml` |

These apps start `pious.exe --mcp`, which passes messages to the running
Pious (and starts Pious in the tray if it isn't running).

## ChatGPT

ChatGPT's connectors run on OpenAI's servers, so they can only reach a
server on the internet, not one on your PC. To use Pious from ChatGPT:

1. Turn on **Let AI apps use Pious** in Pious.
2. Make Pious's server reachable with a tunnel. For example, with
   [Cloudflare's `cloudflared`](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/):

   ```
   cloudflared tunnel --url http://127.0.0.1:47823
   ```

   It prints an address like `https://something.trycloudflare.com`.
3. In Pious, **Copy address** under ChatGPT. It looks like
   `http://127.0.0.1:47823/mcp?token=…`. Swap the start for the tunnel's
   address: `https://something.trycloudflare.com/mcp?token=…`.
4. In ChatGPT's settings, turn on developer mode for connectors, then
   create a connector with that address and no other sign-in.

**The address holds Pious's secret token.** Anyone with it can use Pious
while the tunnel runs. Keep it private, stop the tunnel when you're done,
and turn off **Allow actions** if you only want ChatGPT to look.

To get a new token (and lock out every old address), delete
`mcp-token.txt` in Pious's data folder and restart Pious; reconnect your
apps afterwards.

## Any other app

Under **Set up another app by hand**, Pious shows two ready-made configs:

- **Apps that start a program** (most desktop apps):

  ```json
  { "mcpServers": { "pious": { "command": "C:\\…\\pious.exe", "args": ["--mcp"] } } }
  ```

- **Apps that connect over HTTP** (streamable HTTP transport):

  ```json
  {
    "mcpServers": {
      "pious": {
        "type": "http",
        "url": "http://127.0.0.1:47823/mcp",
        "headers": { "Authorization": "Bearer <token>" }
      }
    }
  }
  ```

  Apps that can't send headers can put the token in the address instead:
  `http://127.0.0.1:47823/mcp?token=<token>`.

The server only listens on this PC (127.0.0.1) and refuses requests from
web pages. Change the port in Settings if 47823 is taken.

## What AI apps can do

### Look (always allowed while the server is on)

| Tool | Does |
| --- | --- |
| `get_status` | What's running, the play-as account, recording and macros. |
| `list_games`, `list_accounts`, `list_friends`, `list_macros`, `get_stats` | What's in Pious. |
| `list_instances` | The Roblox windows running now, and which one is in front. |

### See and hear games (Settings: **See and hear games**)

| Tool | Does |
| --- | --- |
| `screenshot` | A picture of a Roblox window, even behind other windows. |
| `watch` | 2–8 pictures over up to 15 seconds, like a short video. |
| `listen` | Up to 15 seconds of only that game's sound, as WAV audio. |

The AI only gets what it can take in: apps that don't accept pictures or
sound simply won't use these. Minimized windows can't be pictured.

### Act (Settings: **Allow actions**)

| Tool | Does |
| --- | --- |
| `launch_game`, `join_friend`, `join_server_link`, `server_hop`, `close_game` | Start, join, hop and close games. |
| `toggle_recording`, `save_clip` | Record and clip. |
| `run_macro`, `stop_macros` | Your macros. |
| `send_message` | A Roblox chat message to a friend. |
| `focus_instance` | Bring a Roblox window to the front. |
| `move_character` | Walk (W/A/S/D) or jump for a while. |
| `press_keys` | Press a key or a combination, held for a time, a number of times. |
| `type_text` | Type, by default as a chat message (opens chat with `/`, sends with Enter). |
| `click` | Click at a spot given as fractions of the window. |
| `run_input` | Any list of macro steps (keys, text, clicks, scrolling, waits, loops). |

The tools that play take an optional `instance`: a running game's ID from
`list_instances`, its game's name or its account's name. Without it, they
use the Roblox window in front, else the newest one.

Input goes straight to the Roblox window in the background, so your own
mouse and keyboard stay free and the window doesn't need to be in front.
Turning the camera with the mouse can't be done in the background; an AI
can turn with keys a game binds instead (many use the arrow keys or Q/E).

## Turning it off

Turn off **Let AI apps use Pious**. Connected apps then get "Pious isn't
reachable" until it's back on. **Remove** next to an app takes Pious out
of its settings.

## Troubleshooting

- **"Pious isn't reachable."** Turn the server on, and check Settings shows
  "Listening at …".
- **"Port … is in use."** Pick another port, then reconnect the app (or
  update its config).
- **401 from an HTTP app.** The token is missing or old. Copy the config
  again.
- **ChatGPT can't connect.** Check the tunnel is running and the address
  ends in `/mcp?token=…`.
