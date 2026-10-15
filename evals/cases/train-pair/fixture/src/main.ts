import { Config } from "./config";
import { SoapServer } from "./soap-server";
import { KafkaProducer } from "realtime-common/kafka";
import { FastifyAdapter } from "realtime-common/server";

export class Main {
    private server: SoapServer;
    constructor(private webApp: FastifyAdapter, private kafkaProducer: KafkaProducer) {
        this.server = new SoapServer(this.webApp, this.kafkaProducer);
    }

    public async start(): Promise<void> {
        Config.logger.info("Starting Train connector...");
        await this.server.start();
        await this.kafkaProducer.start();
        this.webApp.configureStatusCheck([this.kafkaProducer]);
    }

    public async stop(): Promise<void> {
        Config.logger.info("Stopping Train connector...");
        await this.kafkaProducer.stop();
        await this.server.stop();
    }
}
