import { Command } from "commander";
import { logger } from "./config";
import { importLedger } from "./jobs/import";
import type { Job, JobOptions } from "./jobs/job";
import { runNightly } from "./jobs/nightly";
import { reconcile } from "./jobs/reconcile";
import { pool } from "./lib/db";

// One row per command; the loop below registers each with commander, so
// the table is where a command is declared.
const JOBS: Job[] = [
  { usage: "import <file>", description: "Import a ledger CSV export", run: importLedger },
  { usage: "reconcile <from> <to>", description: "Match ledger entries against the bank feed", run: reconcile },
  { usage: "nightly", description: "Run the overdue sweep once, now", run: runNightly },
];

const program = new Command();

program.name("ops").description("Finance operations").version("0.3.0");

for (const job of JOBS) {
  program
    .command(job.usage)
    .description(job.description)
    .option("--dry-run", "report without writing", false)
    .action(async (...invoked: unknown[]) => {
      // commander passes the positional arguments, then the options, then the command
      const options = invoked.at(-2) as JobOptions;
      const args = invoked.slice(0, -2) as string[];
      process.exitCode = await job.run(args, options);
    });
}

program.hook("postAction", async () => {
  await pool.end();
});

program.parseAsync(process.argv).catch((error: Error) => {
  logger.error({ err: error }, "command failed");
  process.exit(1);
});
