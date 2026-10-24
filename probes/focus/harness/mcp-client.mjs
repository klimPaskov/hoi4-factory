// Minimal newline-delimited JSON-RPC client for an MCP stdio server.
// No dependencies, so the probe does not reuse the server's own SDK copy.
import { spawn } from 'node:child_process';

export class StdioMcpClient {
  constructor(command, args, options) {
    this.child = spawn(command, args, { ...options, stdio: ['pipe', 'pipe', 'pipe'] });
    this.nextId = 1;
    this.pending = new Map();
    this.stderr = '';
    this.notifications = [];
    this.buffer = '';
    this.exited = new Promise((resolve) => {
      this.child.on('exit', (code, signal) => {
        for (const { reject } of this.pending.values()) reject(new Error(`server exited (${code}) before reply`));
        this.pending.clear();
        resolve({ code, signal });
      });
    });
    this.child.stdout.setEncoding('utf8');
    this.child.stdout.on('data', (chunk) => {
      this.buffer += chunk;
      let index;
      while ((index = this.buffer.indexOf('\n')) >= 0) {
        const line = this.buffer.slice(0, index).trim();
        this.buffer = this.buffer.slice(index + 1);
        if (line === '') continue;
        let message;
        try {
          message = JSON.parse(line);
        } catch {
          this.notifications.push({ unparsable: line.slice(0, 2000) });
          continue;
        }
        if (message.id !== undefined && this.pending.has(message.id)) {
          const { resolve } = this.pending.get(message.id);
          this.pending.delete(message.id);
          resolve(message);
        } else if (message.id !== undefined && message.method !== undefined) {
          // Server-to-client request (for example roots/list). Answer with an empty result.
          const reply =
            message.method === 'roots/list'
              ? { jsonrpc: '2.0', id: message.id, result: { roots: [] } }
              : { jsonrpc: '2.0', id: message.id, error: { code: -32601, message: 'not supported by probe' } };
          this.child.stdin.write(`${JSON.stringify(reply)}\n`);
          this.notifications.push({ serverRequest: message.method });
        } else {
          this.notifications.push(message);
        }
      }
    });
    this.child.stderr.setEncoding('utf8');
    this.child.stderr.on('data', (chunk) => {
      this.stderr += chunk;
    });
  }

  request(method, params, timeoutMs = 120000) {
    const id = this.nextId++;
    const payload = { jsonrpc: '2.0', id, method, ...(params === undefined ? {} : { params }) };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`timeout waiting for ${method}`));
      }, timeoutMs);
      this.pending.set(id, {
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
        resolve: (message) => {
          clearTimeout(timer);
          resolve(message);
        },
      });
      this.child.stdin.write(`${JSON.stringify(payload)}\n`);
    });
  }

  notify(method, params) {
    this.child.stdin.write(
      `${JSON.stringify({ jsonrpc: '2.0', method, ...(params === undefined ? {} : { params }) })}\n`,
    );
  }

  async initialize() {
    const response = await this.request('initialize', {
      protocolVersion: '2025-06-18',
      capabilities: {},
      clientInfo: { name: 'hoi4-factory-exp02-probe', version: '0.0.1' },
    });
    this.notify('notifications/initialized');
    return response;
  }

  async close() {
    this.child.stdin.end();
    const timer = setTimeout(() => this.child.kill('SIGKILL'), 5000);
    const result = await this.exited;
    clearTimeout(timer);
    return result;
  }
}
