# Getting Started

## Installation

1. Download `ams2_championship_server.exe` from the [latest release](https://github.com/Nightrat/ams2_championships/releases/latest).
2. Place it in any folder on your PC — for example `C:\AMS2Championships\`.
3. Run it. A `championships\` subfolder is created automatically next to the executable on first launch.

> You do not need to install anything else. The server is a single self-contained executable.

## Starting the server

Double-click `ams2_championship_server.exe`, or run it from a terminal:

```
ams2_championship_server.exe
```

By default it listens on port **8080**. To use a different port:

```
ams2_championship_server.exe 9000
```

You will see output like:

```
Career data:    ...\championships\ams2_career.json (0 championship(s), 0 session(s))
Serving at http://127.0.0.1:8080/  (Ctrl+C to stop)
```

## Opening the UI

Open your browser and go to:

```
http://127.0.0.1:8080/
```

Keep the server running in the background while you play AMS2. Press **Ctrl+C** in the terminal window to stop it.

## First race

1. Start AMS2 and enter a race session.
2. Finish the race (or let it reach the results screen).
3. The server detects the session end automatically and saves the results.
4. Switch to the browser and go to the **Manage** tab to assign the recorded session to a championship.

> The session recorder only captures **race** sessions (not practice or qualifying).
