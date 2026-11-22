# Performance

This document gives the performance budgets of Post and the values that the project measured. Decision record D-170 gives the budgets. The project measures before each minor version.

## The reference computer

The reference computer is a laptop with these parts:

- An Intel Core i5-8365U processor with 4 cores and 8 threads.
- 24 GB of memory.
- Built-in Intel UHD Graphics 620.
- Windows 11.

Other programs were in operation on the computer during the measurements. They used 2% to 11% of the processor.

## Values for version 0.2

The project measured release builds on 2026-11-22.

| Subject | Budget | Measured value | Result |
|---|---|---|---|
| Start of the application | The window shows in 300 ms or less after a warm start. | 61 ms | The budget is met. |
| Memory of the application | The application uses 150 MB or less when idle. | 119 MB | The budget is met. |
| Processor time of the application | The application uses 1% of one processor core or less when idle. | 0.57% | The budget is met. |
| Timeline | A frame takes 16 ms or less with 100,000 messages. | No value | This budget starts in version 0.4. |
| Message delivery | The server sends a message to 1,000 members of a channel in 50 ms or less. | 47 ms to 57 ms | The value is at the limit of the budget. Read the section below. |
| Server memory | 10,000 idle connections use 1 GB or less. | 107 MB | The budget is met. |

## The application

The script `tools/measure-app.ps1` does the measurement on Windows. It uses a temporary directory for the data, thus it does not change your profiles.

- **Start.** The script starts the application 6 times. For each start, it measures the time until the window exists. The first start is not a warm start. The 5 warm starts took 41 ms to 94 ms, and the median was 61 ms.
- **Memory.** The script reads the memory of the application after 8 seconds without input. It uses the Windows value `WorkingSet64`.
- **Processor time.** The window is in front, and the text cursor of the first field is on. The script measures the processor time of 30 seconds without input. It does this 3 times. The values were 0.57%, 1.35%, and 0.47% of one core. The median was 0.57%.

Version 0.1.0 gives the same values for the memory and for the processor time.

A program that the computer starts for the first time after a build can use 2% to 4% of one core for some starts. The value then decreases. Measure only after some starts.

## Message delivery

The test starts the server program and makes 1,000 accounts on the reference computer. Each account has one connection. One member sends a message 30 times. For each message, the test measures the time until all 1,000 connections have the message.

- The first messages after a pause took 23 ms to 29 ms. The processor operates at its burst speed for a short time.
- Later messages took more time. The median of the 30 messages was 47 ms, 53 ms, and 57 ms in three runs. The processor then operates at its continuous power limit.

The 1,000 test clients use the same processor as the server. For each message, the server program used 21 ms of processor time in its own code and 92 ms in the operating system. The time in the operating system is the network work for connections on the same computer. The work of the receiver is a part of this time.

Thus the test does not show the time of a server with members on other computers. That time is smaller.

The nightly checks ran the same test on a Linux computer of GitHub on 2026-11-22. That computer is not the reference computer. The median of the 30 messages was 7 ms. For each message, the server program used 3 ms of processor time in its own code and 10 ms in the operating system. With 10,000 idle connections, the server used 91 MB.

**Open point.** Measure the delivery with the clients on a second computer. Decrease the work for each connection before channels with many members come.

## Server memory

The test starts the server program and opens 10,000 WebSocket connections. The connections do not sign in. A connection that signs in also holds its handle and its role.

- The server used 7 MB without connections.
- The server used 107 MB with 10,000 idle connections. That is 10 kB for each connection.

Before this measurement, each connection used 137 kB, and 10,000 connections used 1,375 MB. The cause was the read buffer of the WebSocket library. The server now gives each connection a read buffer of 4,096 bytes (D-174).

## How to measure the server

```
cargo test --release -p post-server --test budgets -- --ignored --nocapture --test-threads 1
```

The command prints the values. The test for the memory stops with an error if the server uses more than the budget. The test for the delivery prints if the budget is met. It stops with an error only if the median is more than 150 ms.

After one run, wait some minutes before the next run. The operating system keeps the 11,000 network ports of a run for a short time.

The nightly checks run this command on the computers of GitHub. Those computers are not the reference computer, and their speed changes (D-105). Thus only the measurement on the reference computer shows if a budget is met.
