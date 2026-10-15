import * as fs from "fs";
import * as soap from "soap";
import * as moment from "moment-timezone";
import * as express from "express";
import * as x2js from "x2js";
import { APMService } from "realtime-common/apm";

import { Config } from "./config";
import { EventStore } from "connector-common";
import { FastifyAdapter } from "realtime-common/server";
import { KafkaProducer } from "realtime-common/kafka";
import { TrainEvent } from "./types/train-event";
import { ERROR_RESPONSE, NOT_IMPLEMENTED, OK_RESPONSE } from "./types/response";

const ROOT_ELEMENT = "CCO";
const CHANGES_PATH = ROOT_ELEMENT + ".ActualizarDatosTren.pasoTren";

const XML_OPTIONS = {
    arrayAccessFormPaths: [CHANGES_PATH],
};

export class SoapServer {
    private eventStore = new EventStore(Config.logger, Config.replication.endpoint, "train");
    private soapService = {
        IwsTrainservice: {
            IwsTrainPort: {
                ReceiveMessage: this.receiveMessage.bind(this),
                Login: () => {
                    throw NOT_IMPLEMENTED;
                },
                UploadMessages: () => {
                    throw NOT_IMPLEMENTED;
                },
            },
        },
    };

    constructor(private readonly webApp: FastifyAdapter, private publisher: KafkaProducer) {}

    public async start() {
        const wsdl = fs.readFileSync(Config.wsdlPath, "utf8");
        soap.listen(this.webApp.getHttpServer(), "/inbound/xml", this.soapService, wsdl);
    }

    public async stop() {
        await this.webApp.close();
    }

    private async receiveMessage(args: { AXMLMessage: string }, _cb: unknown, _headers: unknown, req: express.Request) {
        APMService.incrementMetric(`${Config.apm.appName}/message_counter`, 1);

        const receivedMessage = args.AXMLMessage;

        if (Config.replication.endpoint && receivedMessage) {
            this.eventStore.put(receivedMessage);
        }

        Config.logger.debug(receivedMessage);

        if (!receivedMessage) {
            APMService.noticeError(new Error("AXMLMessage is empty: "));
            Config.logger.error("AXMLMessage is empty: " + receivedMessage);
            throw ERROR_RESPONSE;
        }

        let parsed: TrainEvent;
        try {
            parsed = new x2js(XML_OPTIONS).xml2js(receivedMessage);
            if (Object.keys(parsed).length !== 1 || !parsed?.CCO) {
                throw ERROR_RESPONSE;
            }

            if (!parsed.CCO.MensajeVidaSIE) {
                let eventSeconds = 0;
                const currentUpdate = parsed.CCO.ActualizarDatosTren?.pasoTren?.[0];

                if (currentUpdate?.haEntrado === "true") {
                    eventSeconds = Number(currentUpdate.horaEntradaReal);
                }

                if (currentUpdate?.haSalido === "true") {
                    eventSeconds = Number(currentUpdate.horaSalidaReal);
                }

                if (eventSeconds > 0) {
                    const messageTime = moment.tz(parsed.CCO?.ActualizarDatosTren?.fechaCreacion as string, "DD/MM/YYYY", Config.timezone).unix() + eventSeconds;
                    APMService.incrementMetric(`${Config.apm.appName}/message_delay`, Date.now() - messageTime * 1000);
                }
            }
        } catch (error) {
            APMService.noticeError(error);
            Config.logger.error("Invalid request received: " + receivedMessage);
            throw ERROR_RESPONSE;
        }

        // Publish the event as XML to downstream consumers and Train Position Adapter
        await this.publisher
            .publish((await Config.getKafkaProducerConfig()).producer.topic, {
                value: receivedMessage,
                key: parsed.CCO.ActualizarDatosTren?.trenPar || undefined,
            })
            .catch((error: Error) => {
                Config.logger.error(error.stack || error.message);
                APMService.noticeError(error);
            });

        return OK_RESPONSE;
    }
}
