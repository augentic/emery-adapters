import fetch, { Response } from "node-fetch";

import { ServerFactory } from "realtime-common/server";
import { ConfluentSecretRetriever } from "../../../src/confluent-secret-retriever";
import { Config } from "../../../src/config";
import { KafkaConsumer, KafkaMessage, KafkaProducer, MockKafka } from "realtime-common/kafka";
import { Main } from "../../../src/main";

let nextTestPort = 3101;

function allocateTestPort(): string {
    nextTestPort += 1;
    return String(nextTestPort);
}

export type SoapServerHarness = {
    main: Main;
    kafkaReceiver: KafkaConsumer;
    kafkaMessages: KafkaMessage[];
    waitForKafkaMessage: (timeoutMs?: number) => Promise<KafkaMessage | undefined>;
    sendSoapRequest: (body: string) => Promise<Response>;
    stop: () => Promise<void>;
};

export async function createSoapServerHarness(options?: {
    replicationEndpoint?: string;
    kafkaProducer?: KafkaProducer;
}): Promise<SoapServerHarness> {
    const replicationEndpoint = options?.replicationEndpoint ?? "";
    process.env.REPLICATION_ENDPOINT = replicationEndpoint;
    Config.replication.endpoint = replicationEndpoint;

    ConfluentSecretRetriever.confluentKafkaSecret = { username: "", password: "" };
    const kafkaProducerConfig = await Config.getKafkaProducerConfig();

    const kafkaMessages: KafkaMessage[] = [];
    const kafkaReceiver = new MockKafka(
        Config.logger,
        {},
        {
            brokers: kafkaProducerConfig.producer.endpoint,
            topics: kafkaProducerConfig.producer.topic,
        },
    );
    kafkaReceiver.onMessage((message: KafkaMessage) => {
        kafkaMessages.push(message);
    });
    await kafkaReceiver.start();

    const webApp = await ServerFactory.createSimple(Config.logger);
    const kafkaProducer = options?.kafkaProducer ?? new MockKafka(Config.logger, {}, {
        brokers: kafkaProducerConfig.producer.endpoint,
        on: () => ({}),
    } as never);

    const main = new Main(webApp, kafkaProducer);
    const testPort = allocateTestPort();
    Config.port = testPort;
    await main.start();
    webApp.listen(testPort);

    return {
        main,
        kafkaReceiver,
        kafkaMessages,
        waitForKafkaMessage: (timeoutMs = 500) =>
            new Promise((resolve) => {
                if (kafkaMessages.length > 0) {
                    resolve(kafkaMessages[0]);
                    return;
                }

                const startedAt = Date.now();
                const interval = setInterval(() => {
                    if (kafkaMessages.length > 0) {
                        clearInterval(interval);
                        resolve(kafkaMessages[0]);
                        return;
                    }
                    if (Date.now() - startedAt >= timeoutMs) {
                        clearInterval(interval);
                        resolve(undefined);
                    }
                }, 10);
            }),
        sendSoapRequest: (body: string) =>
            fetch(`http://localhost:${Config.port}/inbound/xml`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/xml",
                },
                body,
            }),
        stop: async () => {
            await kafkaReceiver.stop();
            await main.stop();
        },
    };
}
