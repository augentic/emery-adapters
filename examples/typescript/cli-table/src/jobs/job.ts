export interface JobOptions {
  dryRun: boolean;
}

// A row of the command table in cli.ts: the usage line commander parses, the
// help text, and what runs — answering the process exit code.
export interface Job {
  usage: string;
  description: string;
  run: (args: string[], options: JobOptions) => Promise<number>;
}
