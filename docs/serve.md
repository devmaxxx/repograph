# Asking a resident process: serve

Most of a fused `ask` is the process opening things it then throws away: the embedding model
alone costs about 220 ms of it on the shipped default. `serve` opens
them once and answers over a Unix socket — on Windows too, where the same socket file has existed
since Windows 10 1803 and is protected the way the repository directory is:

```bash
repograph serve                  # .repograph/serve.sock, poll every 30 s, exit after 30 min idle
repograph serve --idle 86400     # a day rather than half an hour before it gives up
repograph serve --idle-model 60  # forget the model after a minute unasked; the process stays
repograph serve --no-dense       # lexical only; a fused `ask` is told so and answers in its own process
repograph ask --no-serve отмена  # answer here even while one is listening
```

Two idles, one clock. `--idle` ends the process; `--idle-model` (default 300 s) drops only the
model weights and keeps everything that answers without them — the graph, the ids, the lexical
indexes, the vectors — so a server left up overnight is cheap to leave up. On a 33.5k-row store the
resident size goes 908.5 MB → 28.8 MB on the first drop and 277.6 MB on later ones (the allocator
keeps some of what the reopen took), and the first fused `ask` after a drop pays the open: 0.771 s
against 0.083 s warm. The local reranker is held the same way: `serve` opens the cross-encoder the
first time an `ask --rerank-local` asks for it, keeps it between questions and drops it on the same
`--idle-model`.

`ask --rerank` and `ask --rerank-local` are both answered by the resident process, and a client
waits five minutes rather than thirty seconds for either — a local rerank of a 200-deep pool runs
70-80 s cold on CPU, and a client that gave up at thirty seconds would score the same pool itself
behind its own cold open, paying the whole cost a second time. The five minutes are bought: the
thread that accepts a connection writes an acknowledgement on a line of its own straight away,
before the question is even read, and the client waits thirty seconds for that line. Accepting is
what the server can promise while it is still answering somebody else, so a question queued behind
a 70-80 s rerank keeps its five minutes. A server that accepted the connection and will never
answer — a wedged worker, a process stopped under a debugger — hands the question back as
promptly as it does for every other request.

The socket lives at `.repograph/serve.sock` — unless that path would be longer than a Unix socket
name may be (104 bytes on macOS, including the terminating NUL), in which case it goes in the
temporary directory as `repograph-<hash of the repository path>.sock`. A repository under a deep
enough path could not run `serve` at all before that fallback, and both the server and every client
compute the name the same way, so nothing has to be told which one is in use — the startup line
prints it. A `serve` that is killed with `SIGTERM` or `SIGINT` takes its socket file with it,
because it leaves through the same exit as an idle timeout; on Windows there is no equivalent
signal and the file is left for the next `serve`, which removes a dead socket before it binds.

`ask` uses it without being told to and answers in this process whenever it cannot — no socket, a
socket nobody listens on, a server of another version or another build of the same version, a
`--no-dense` server asked a fused question, a timeout, `--no-serve`, or `REPOGRAPH_NO_SERVE`. Each
of those prints a line: a question that quietly costs a cold process, or quietly gets a lexical
answer, looks like nothing at all. A client never deletes the socket file, since a refused connect
is also what a live server with a full backlog gives; only `serve` removes one, and only the one it
bound itself — told from a replacement's by device and inode on unix, by NTFS's file reference
number on Windows.

The socket name is bounded by the platform's own limit — 104 bytes on macOS, 108 elsewhere — which
is why the fallback exists; the arithmetic of a deep Windows path, and the check that a resident
answer is byte-identical to a cold one across all 142 recorded questions, are in
[the measurements](history.md#the-socket-path-and-the-same-bytes-either-way).

The server refreshes before every answer with the same walk a one-shot `ask` does, and polls
between them like `watch`, so its **graph** is never staler than a fresh process's; `--stale` skips
that walk and reads the stored graph as it is on disk. It answers one question at a time, and a
second client waits rather than being turned away. Both reads are decided by `manifest.json`, so a
running server does not see `enrich` writing `questions.json` or `embed` writing `vectors.*` — stop
it, or ask with `--no-serve`, to answer from the new ones.

The configuration is read once, at start-up. Editing `repograph.toml` stops the server after its
next poll — the next `ask` answers in its own process under the new file, and a new `serve` starts
under it too.

Measured on the bench corpus, median of eleven with socket and in-process runs interleaved: a fused
question is **66 ms** resident against 326 ms in a fresh process, and the lexical arm **6.8 ms**
against 106 ms once the BM25 indexes stopped being rebuilt per question. The first question after a
start still pays the model open. The full tables, and the evidence that the answer bytes do not
move, are in [the measurements](history.md#the-resident-answer-measured).

