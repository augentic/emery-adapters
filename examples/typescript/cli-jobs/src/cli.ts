import { Command } from "commander";
import { logger } from "./config";
import { pool } from "./lib/db";
import { runNightly, scheduleNightly } from "./jobs/nightly";
import { connection, invoiceQueue } from "./queues";
import { importLedger } from "./services/importer";
import { reconcile } from "./services/reconcile";
import { startInvoiceWorker } from "./workers/invoices";

const program = new Command();

program.name("ledger-tool").description("Ledger import, reconciliation, and invoice processing").version("0.9.1");

program
  .command("import <file>")
  .description("Import a ledger CSV export")
  .option("--dry-run", "parse and report without writing", false)
  .action(async (file: string, options: { dryRun: boolean }) => {
    const summary = await importLedger(file, options.dryRun);
    if (summary.rejected > 0 && !options.dryRun) {
      process.exitCode = 2;
    }
    console.log(JSON.stringify(summary));
  });

program
  .command("reconcile")
  .description("Match ledger entries against the bank feed for a date range")
  .requiredOption("--from <date>", "first posting date, inclusive")
  .requiredOption("--to <date>", "last posting date, exclusive")
  .option("--import", "import unmatched bank movements into the ledger", false)
  .action(async (options: { from: string; to: string; import: boolean }) => {
    const result = await reconcile(options.from, options.to, options.import);
    console.log(JSON.stringify({ ...result, unmatchedLedger: result.unmatchedLedger.length, unmatchedBank: result.unmatchedBank.length }));
    if (result.unmatchedLedger.length > 0 || result.unmatchedBank.length > 0) {
      process.exitCode = 3;
    }
  });

program
  .command("nightly")
  .description("Run the nightly reconciliation and overdue sweep once, now")
  .action(async () => {
    await runNightly();
  });

program
  .command("serve")
  .description("Run the invoice worker and the nightly schedule until stopped")
  .action(() => {
    const worker = startInvoiceWorker();
    scheduleNightly();
    const stop = async (signal: string) => {
      logger.info({ signal }, "stopping");
      await worker.close();
      await invoiceQueue.close();
      connection.disconnect();
      await pool.end();
      process.exit(0);
    };
    process.on("SIGINT", () => void stop("SIGINT"));
    process.on("SIGTERM", () => void stop("SIGTERM"));
    logger.info("ledger-tool serving");
  });

program.hook("postAction", async (thisCommand) => {
  if (thisCommand.name() !== "serve") {
    await pool.end();
    connection.disconnect();
  }
});

program.parseAsync(process.argv).catch((error: Error) => {
  logger.error({ err: error }, "command failed");
  process.exit(1);
});
