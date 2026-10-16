import express from "express";
import { Kafka } from "kafkajs";
import { Config } from "./config";
import { ConfluentSecretRetriever } from "./confluent-secret-retriever";
import { ordersRouter } from "./orders";

async function run() {
    if (Config.useConfluentKafkaConfig) {
        await ConfluentSecretRetriever.retrieve();
    }

    const kafka = new Kafka({
        brokers: Config.kafka.brokers,
        ssl: Config.useConfluentKafkaConfig,
        sasl: Config.useConfluentKafkaConfig
            ? { mechanism: "plain", ...ConfluentSecretRetriever.confluentKafkaSecret }
            : undefined,
    });
    const producer = kafka.producer();
    await producer.connect();

    const app = express();
    app.use(express.json());
    app.use(ordersRouter(producer));
    const server = app.listen(Config.port);

    if (Config.useConfluentKafkaConfig) {
        await ConfluentSecretRetriever.watch(() => {
            server.close();
            producer.disconnect();
            setTimeout(() => process.exit(1), Config.restartDelayMs);
        });
    }
}

run();
