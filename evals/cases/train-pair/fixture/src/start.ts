import { APMService } from "realtime-common/apm";
import { Main } from "./main";
import { Config } from "./config";
import { ServerFactory } from "realtime-common/server";
import { KafkaProducer } from "realtime-common/kafka";
import { ConfluentSecretRetriever } from "./confluent-secret-retriever";

let main: Main;

async function start() {
    await APMService.initialize(Config);
    Config.logger.info(`Train Connector is listening on port ${Config.port}`);
    const webApp = await ServerFactory.createSimple(Config.logger);

    const kafkaProducerConfig = await Config.getKafkaProducerConfig();
    const kafkaProducer = new KafkaProducer(Config.logger, { newRelicPrefix: Config.apm.appName }, {
        brokers: kafkaProducerConfig.producer.endpoint,
        topic: kafkaProducerConfig.producer.topic,
        ssl: kafkaProducerConfig.producer.ssl,
        sasl: kafkaProducerConfig.producer.sasl,
        on: () => ({}),
    } as never);
    main = new Main(webApp, kafkaProducer);

    await main.start();
    webApp.listen(Config.port);
    APMService.incrementMetric(`${Config.apm.appName}/startup`);

    if (Config.useConfluentKafkaConfig) {
        await ConfluentSecretRetriever.watch(() => {
            Config.logger.info("Restarting application to reload Kafka keys");
            closeApp();
        });
    }
}

const closeApp = () => {
    // do not await stop as it could get stuck
    main?.stop();
    return setTimeout(() => process.exit(1), 10 * 1000);
};

const errorTypes = ["unhandledRejection", "uncaughtException"];
errorTypes.forEach((errorType) => {
    process.on(errorType, async (err: Error) => {
        APMService.noticeError(err);
        Config.logger.error(`Received '${errorType}': ${err.stack || err}`);
        closeApp();
    });
});

start();
