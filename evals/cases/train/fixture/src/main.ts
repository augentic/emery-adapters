import * as X2JS from "x2js";
import * as moment from "moment-timezone";

import { Config } from "./config";
import { Message } from "./train-update";
import { GtfsApi } from "./services/gtfs-api";
import { BlockMgtApi } from "./services/block-mgt-api";
import { PositionConverter } from "./position-converter";
import { JSON as TypedJSON } from "ta-json";
import { VehicleEvent } from "connector-common";
import { ApmCommon, ExpressCommon, KafkaCommon } from "realtime-common";
import { ValidationError } from "./types";

const { APMService } = ApmCommon;

export class Main {
    private x2js: X2JS;
    private gtfsApi: GtfsApi;
    private blockMgtApi: BlockMgtApi;
    private positionConverter: PositionConverter;

    private messagesSent = 0;
    private messagesReceived = 0;
    private scheduleLogTimeout: NodeJS.Timeout;

    constructor(private webApp: ExpressCommon.ExpressWebAppWrapper, private consumer: KafkaCommon.KafkaConsumer, private producer: KafkaCommon.KafkaProducer) {
        this.x2js = new X2JS(Config.xmlOptions);
        this.gtfsApi = new GtfsApi();
        this.blockMgtApi = new BlockMgtApi();
        this.positionConverter = new PositionConverter(this.gtfsApi, this.blockMgtApi);
    }

    public async start() {
        Config.logger.info("Starting Train Position adapter..");

        this.consumer.onMessage(async (message, next) => {
            await APMService.startBackgroundTransaction("onMessage", async () => {
                await this.processMessage(message)
                    .catch((err) => {
                        APMService.noticeError(err);
                        Config.logger.error(`Processing message '${message.value}': ${err.stack || err.message}`);
                    });
                next();
            });
        });

        await this.blockMgtApi.fetchAuthToken();
        await this.gtfsApi.fetchStops();
        await this.producer.start();

        if (!Config.isSlot) {
            await this.consumer.start();
        }

        this.webApp.configureStatusCheck([this.consumer, this.producer]);
        this.scheduleStatusLog();
        Config.logger.info("Train Position adapter has been started..");
    }

    public async stop() {
        Config.logger.info("Stopping Train Position adapter..");
        await this.webApp.stop();
        await this.consumer.stop();
        await this.producer.stop();
        clearTimeout(this.scheduleLogTimeout);
        Config.logger.info("Train Position adapter has been stopped..");
    }

    private async processMessage({ value }: { value: string, topic: string }) {
        this.messagesReceived++;
        let message: Message | undefined;
        try {
            message = this.deserializeAndValidate(value);
        } catch (err) {
            Config.logger.warn(err.message);
            APMService.incrementMetric(`${Config.apmPrefix}/invalid_message_counter_${err.errorType || "unknown"}`, 1);
        }

        if (!message) {
            return;
        }
        const vehicleEvents = await this.positionConverter.convert(message.trainUpdate);
        await Promise.all(vehicleEvents.map(async (event: VehicleEvent) => {
            const twoTap = async () => {
                // This twoTap is used for schedule adherence to depart vehicle from the station properly
                const FIVE_SEC_DELAY = 5 * 1000;

                await this.sleep(FIVE_SEC_DELAY);
                event.messageData.timestamp = moment(event.messageData.timestamp).add(FIVE_SEC_DELAY, "ms").toDate();
                await this.publish(event);

                await this.sleep(FIVE_SEC_DELAY);
                event.messageData.timestamp = moment(event.messageData.timestamp).add(FIVE_SEC_DELAY, "ms").toDate();
                await this.publish(event);
            };

            twoTap();
        }));
    }

    private deserializeAndValidate(messageStr: string): Message {
        const object = this.x2js.xml2js<{ [key: string]: unknown }>(messageStr);
        const message = TypedJSON.deserialize<Message>(object[Config.rootElement], Message);
        if (!message?.trainUpdate?.changes?.length) {
            throw new ValidationError("no_update", `Received invalid message, reason:  no_update ${messageStr}`);
        }

        const actualTrainUpdate = message.trainUpdate.changes[0];
        let eventSeconds = 0;
        if (actualTrainUpdate.hasDeparted) {
            eventSeconds = actualTrainUpdate.actualDepartureTime;
        } else if (actualTrainUpdate.hasArrived) {
            eventSeconds = actualTrainUpdate.actualArrivalTime;
        }
        if (eventSeconds <= 0) {
            throw new ValidationError("no_actual_update", `Received invalid message, reason:  no_actual_update ${messageStr}`);
        }

        const eventDate = moment.tz(message.trainUpdate.createdDate, Config.dateFormat, Config.timezone);
        const messageDelay = Math.floor(Date.now() / 1000) - (eventDate.unix() + eventSeconds);
        APMService.recordMetric(`${Config.apmPrefix}/message_delay`, messageDelay);

        if (messageDelay > Config.maxMessageDelay) {
            throw new ValidationError("outdated", `Received invalid message, reason: outdated, delay in seconds: ${messageDelay} ${messageStr}`);
        }

        if (messageDelay < Config.minMessageDelay) {
            throw new ValidationError("wrong_time", `Received invalid message, reason: wrong time, delay in seconds: ${messageDelay} ${messageStr}`);
        }

        return message;
    }

    private async sleep(milliseconds: number) {
        return new Promise(resolve => setTimeout(resolve, milliseconds));
    }

    private async publish(vehicleEvent: VehicleEvent) {
        this.messagesSent++;
        return this.producer.publish({ value: JSON.stringify(vehicleEvent), key: vehicleEvent.remoteData.externalId || vehicleEvent.remoteData.remoteName })
            .catch((err: Error) => {
                APMService.noticeError(err);
                Config.logger.error(`Could not publish ${JSON.stringify(vehicleEvent)} due to: ${err.stack || err.message}`);
            });
    }

    private scheduleStatusLog() {
        const startTime = new Date().getTime();
        this.scheduleLogTimeout = setTimeout(() => {
            const queueSize = this.producer.queueSize();
            const partitionsAssigned = JSON.stringify(this.consumer.getAssignedPartitions());

            Config.logger.info(`Elapse: ${new Date().getTime() - startTime}ms ` +
                `(sent: ${this.messagesSent} | received: ${this.messagesReceived} | publishQueue: ${queueSize} | partitions: ${partitionsAssigned})`);

            this.messagesSent = 0;
            this.messagesReceived = 0;

            this.scheduleStatusLog();
        }, 60000);
    }
}
