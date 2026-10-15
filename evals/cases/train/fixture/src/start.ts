import { Main } from "./main";
import { Config } from "./config";
import { ApmCommon, ExpressCommon, KafkaCommon } from "realtime-common";
import { ConfluentSecretRetriever } from "./confluent-secret-retriever";

const { APMService } = ApmCommon;

let main: Main;

async function run() {
    if (Config.useConfluentKafkaConfig){
        await ConfluentSecretRetriever.retrieve();
    }

    await APMService.initialize(Config);
    const webApp = new ExpressCommon.ExpressWebAppWrapper(Config.logger);
    await webApp.start(Config.port);

    APMService.incrementMetric(`${Config.apmPrefix}/startup`);

    const kafkaConsumer = new KafkaCommon.KafkaConsumer(
        Config.logger,
        Config.kafka.consumer.endpoints,
        Config.kafka.consumer.topics,
        Config.kafka.consumer.consumerGroup,
        {
            apmPrefix: Config.apmPrefix,
            ssl: Config.useConfluentKafkaConfig,
            sasl: Config.useConfluentKafkaConfig ? {
                mechanism: "plain",
                ...ConfluentSecretRetriever.confluentKafkaSecret,
            }: undefined,
        }
    );

    const kafkaProducer = new KafkaCommon.KafkaProducer(
        Config.logger,
        Config.kafka.producer.endpoints,
        Config.kafka.producer.vpTopic,
        {
            apmPrefix: Config.apmPrefix,
            ssl: Config.useConfluentKafkaConfig,
            sasl: Config.useConfluentKafkaConfig ? {
                mechanism: "plain",
                ...ConfluentSecretRetriever.confluentKafkaSecret,
            } : undefined,
        }
    );

    main = new Main(webApp, kafkaConsumer, kafkaProducer);
    if (Config.useConfluentKafkaConfig){
        await ConfluentSecretRetriever.watch(() => {
            Config.logger.info("Restarting application to reload Kafka keys");
            // do not await stop as it could get stuck
            main.stop();
            return setTimeout(() => process.exit(1), 10 * 1000);
        });
    }

    await main.start();

    Config.logger.info("Status endpoint is running");
}

["unhandledRejection", "uncaughtException"].forEach((errorType: never) => {
    process.on(errorType, async (err: Error) => {
        try {
            APMService.noticeError(err);
            Config.logger.error(`Received '${errorType}': ${err.stack || err}`);
            if (main) {
                main.stop();
            }
        } finally {
            setTimeout(() => process.exit(1), 30000);
        }
    });
});

run();
