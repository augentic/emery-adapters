import { join } from "path";
import { Logger } from "realtime-common/logger";
import { SASLOptions } from "kafkajs";
import { ConfluentSecretRetriever } from "./confluent-secret-retriever";
import { ConfluentTopics, Versions, getConfluentTopic } from "realtime-common/kafka";

export class Config {
    public static port = process.env.PORT || "3000";
    public static appName = process.env.APP_NAME || "train-connector";
    public static apm = {
        packageVersion: process.env.npm_package_version,
        otelEndpoint: process.env.OTEL_ENDPOINT,
        otelToken: process.env.OTEL_TOKEN,
        appName: Config.appName,
        nodeEnv: process.env.NODE_ENV,
    };
    public static isSlot = process.env.IS_SLOT === "true";
    public static timezone = "Pacific/Auckland";

    public static logger = new Logger(this, {
        prefix: process.env.IS_SLOT === "true" ? "[SLOT]" : "",
        level: process.env.LOG_LEVEL || "info",
        system: process.env.PAPERTRAIL_SYSTEM || "",
    });
    public static wsdlPath = join(__dirname, "../resource/wsdl/train.wsdl");
    public static useConfluentKafkaConfig = process.env.IS_LOCAL !== "true";

    public static replication = {
        endpoint: process.env.REPLICATION_ENDPOINT || "",
    };

    public static getKafkaProducerConfig = async () => {
        if (Config.useConfluentKafkaConfig) {
            await ConfluentSecretRetriever.retrieve();
        }
        const baseTopicName = (ConfluentTopics.TRAIN + (process.env.IS_DORMANT === "true" ? ".dormant" : "")) as ConfluentTopics;

        return {
            producer: {
                endpoint: (process.env.KAFKA_HOSTS || "kafka.transit.example.com:9092").split(","),
                topic: getConfluentTopic(baseTopicName, process.env.KAFKA_ENVIRONMENT || "dev", Versions.V1),
                ssl: Config.useConfluentKafkaConfig,
                sasl: Config.useConfluentKafkaConfig
                    ? ({
                          mechanism: "plain",
                          ...ConfluentSecretRetriever.confluentKafkaSecret,
                      } as SASLOptions)
                    : undefined,
            },
        };
    };
}
