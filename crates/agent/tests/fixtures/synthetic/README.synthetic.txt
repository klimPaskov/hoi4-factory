Synthetic fixtures. NOT recorded provider output.

openai-synthetic-*.sse were written by hand in the Chat Completions chunk format
because no real OpenAI stream could be recorded (the OpenAI account has no API
balance). They exercise stream parsing and request/response plumbing only and are
never evidence of OpenAI model behavior. Recorded real data lives in ../recorded/.
