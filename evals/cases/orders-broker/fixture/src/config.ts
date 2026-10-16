export const Config = {
    port: Number(process.env.PORT) || 8080,
    useConfluentKafkaConfig: process.env.IS_LOCAL !== "true",
    keyVaultHost: "https://" + process.env.KEY_VAULT + ".vault.azure.net",
    brokerSecretName: process.env.BROKER_SECRET_NAME || "order-events-broker",
    secretWatcherMinutes: Number(process.env.SECRET_WATCHER_INTERVAL) || 5,
    restartDelayMs: 10 * 1000,
    kafka: {
        brokers: (process.env.KAFKA_BROKERS || "localhost:9092").split(","),
        topic: process.env.ORDER_EVENTS_TOPIC || "order-events",
    },
};
