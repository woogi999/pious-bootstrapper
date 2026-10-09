# Privacy Policy

_Last updated: October 9, 2026_

This policy covers the Pious desktop app, its installer (Pious Setup) and
this repository. Pious is a hobby project, made and published by its
developer (called "we" below). It isn't made by, affiliated with, or
endorsed by Roblox Corporation.

The short version: **Pious runs on your PC and keeps your data on your PC.
It has no accounts, no analytics, no ads and no servers of its own.** It
only talks to the services it needs to do what you ask it to.

## What Pious keeps, and where

Everything stays on your computer:

| What | Where |
| --- | --- |
| Your games, private servers, accounts list (names and user IDs), versions, settings, macros and keybinds | `data\bootstrapper.json` in Pious's install folder (or `%LOCALAPPDATA%\Pious\Bootstrapper` for a copy that wasn't installed) |
| Roblox sign-ins (session tokens) | Windows Credential Manager only, never in a file |
| Play statistics (play time, sessions, launches…) | `data\stats.jsonl`, used only for the summary in Settings → About |
| Cached pictures, friends lists, chats and build lists | `data\cache` |
| Crash reports | `data\crashes` (see below) |
| Interface errors | `data\ui-errors.log` |
| Recordings and clips | Your Videos folder (or the folder you chose) |

You can delete any of it at any time. Uninstalling with **Remove my data**
checked deletes the data folder; removing an account in Pious deletes its
sign-in from Credential Manager.

## Your Roblox sign-in

Pious never asks for your password. When you sign in, you type it into
Roblox's own page. Pious keeps only the session Roblox gives back, in
Windows Credential Manager, and **sends it only to Roblox** (roblox.com
and its subdomains), to do what you asked: launch games, list friends,
chat, join servers and so on.

If you add an account **from your browser**, Pious reads only the
roblox.com sign-in from that browser's cookie store on your PC. Nothing
else in your browser is read, and nothing leaves your PC except to Roblox.

## Who Pious talks to

Pious contacts these services, only for the features that need them:

| Service | Why | What's sent |
| --- | --- | --- |
| Roblox (roblox.com, rbxcdn.com and their subdomains) | Games, accounts, friends, chat, presence, server lists, joining servers, Roblox versions, thumbnails | What Roblox needs for each request, including your session for signed-in requests |
| GitHub (github.com, api.github.com, raw.githubusercontent.com) | Pious's updates and installer downloads; ready-made fonts, cursors, sounds and icons from open-source projects (Fishstrap's resources, Google Fonts, emoji sets) | Ordinary download requests |
| weao.xyz | The list of current and upcoming Roblox builds (Versions) | An ordinary request |
| ipinfo.io | Where a game server is ("the server is in…"), and picking servers by region | The **server's** address, never yours or your account |
| jsDelivr (cdn.jsdelivr.net) | Emoji pictures, when you pick that emoji style | An ordinary download request |
| gyan.dev | FFmpeg for recording, only for a Pious built without it (releases have it built in) | An ordinary download request |
| Discord (on your PC only) | Rich Presence, when you turn it on | What you're playing, sent to the Discord app on your own PC, which shows it on your profile |

Every service above sees the ordinary things any internet request shows,
like your IP address, and their own privacy policies apply to them.

## AI apps (MCP)

If you turn on **Let AI apps use Pious**, AI apps you connect (Claude,
Cursor, Codex, ChatGPT through a tunnel you run…) can see and do what the
settings allow: your games, accounts, friends, running games, and (when
allowed) pictures and sound of Roblox windows. That data goes to the AI
app you connected, under **its** privacy policy. The server only listens
on your own PC and needs a secret token. It's off by default, and AI apps
never start Pious on their own unless you turn that on.

## Plugins and themes

Plugins and themes you install are made by other people. Plugin pages run
walled off from Pious and only get what their permissions allow: without
the **full** permission they never see your sign-ins, passwords or files.
A plugin given **full**, **run** or **input** can do much more (use
everything Pious can do, open programs, type and click), and what it does
with your data is up to its author, not us. **We don't review third-party
plugins or themes and aren't responsible for data they collect, send or
damage, or for accounts they compromise.** Only install ones you trust,
and check their permissions and files first.

## Crash reports

When Pious or a Roblox window it started crashes, Pious writes a short
report to `data\crashes`: the Pious and Windows versions, what was being
played, and the end of the relevant log. Your Windows user name is
replaced with `%USERPROFILE%`, and anything that looks like a Roblox
session is cut out. **Reports never leave your PC by themselves.** If you
choose to share one (for example in a GitHub issue), read it first. You
can turn crash reports off in Settings → About.

## Children

Pious isn't directed at children under 13 and doesn't knowingly collect
anything from anyone; it has no servers to collect anything with. Roblox's
own rules and parental controls still apply to Roblox.

## Changes

If this policy changes, the new version ships with Pious (Help → Privacy
Policy) and in this repository, with a new date at the top.

## Contact

Questions about privacy: open an issue at
<https://github.com/woogi999/pious-bootstrapper/issues>.
