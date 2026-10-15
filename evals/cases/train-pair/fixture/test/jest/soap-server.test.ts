import { EventStore } from "connector-common";
import { APMService } from "realtime-common/apm";
import { MockKafka } from "realtime-common/kafka";

import { Config } from "../../src/config";
import { createReceiveMessageRequest } from "./helper/helper";
import { createSoapServerHarness } from "./helper/soap-server-harness";
import {
    HEARTBEAT_CCO_XML,
    SAMPLE_CCO_XML,
    wrapCcoInReceiveMessageSoap,
    wrapLoginSoapRequest,
    wrapUploadMessagesSoapRequest,
} from "./mock-data/sample-cco-xml";
import { XmlEvents } from "./mock-data/xml-events";

const mockEventStorePut = jest.fn<void, [string]>();
const replicationEndpoint = "http://message-replicator.test/providers/train/messages";
const previousReplicationEndpoint = process.env.REPLICATION_ENDPOINT;

jest.mock("connector-common", () => ({
    EventStore: jest.fn().mockImplementation(() => ({
        put: mockEventStorePut,
    })),
}));

jest.mock("realtime-common/apm", () => ({
    APMService: {
        noticeError: jest.fn(),
        initialize: jest.fn(),
        incrementMetric: jest.fn(),
        fastifyOtelInstrumentation: null,
    },
}));

jest.mock("@azure/keyvault-secrets", () => ({
    SecretClient: jest.fn(() => ({
        getSecret: async () => ({
            value: JSON.stringify({
                key: "test",
                secret: "password",
            }),
        }),
    })),
}));

describe("SoapServer", () => {
    beforeEach(() => {
        jest.clearAllMocks();
        mockEventStorePut.mockClear();
    });

    afterEach(() => {
        process.env.REPLICATION_ENDPOINT = previousReplicationEndpoint;
        Config.replication.endpoint = previousReplicationEndpoint || "";
    });

    describe("ReceiveMessage validation", () => {
        it("ignores non-SOAP HTTP bodies without processing the message", async () => {
            const harness = await createSoapServerHarness();
            const debugLogSpy = jest.spyOn(Config.logger, "debug");

            try {
                await harness.sendSoapRequest("invalid content");

                expect(debugLogSpy).not.toHaveBeenCalled();
                expect(harness.kafkaMessages).toHaveLength(0);
            } finally {
                await harness.stop();
            }
        });

        it("rejects SOAP requests with an empty AXMLMessage", async () => {
            const harness = await createSoapServerHarness({ replicationEndpoint });
            const errorLogSpy = jest.spyOn(Config.logger, "error");

            try {
                await harness.sendSoapRequest(XmlEvents.EmptyAXMLMessage);

                expect(errorLogSpy).toHaveBeenCalledWith(expect.stringContaining("AXMLMessage is empty"));
                expect(APMService.noticeError).toHaveBeenCalled();
                expect(APMService.incrementMetric).toHaveBeenCalledWith(
                    expect.stringContaining("message_counter"),
                    1,
                );
                expect(mockEventStorePut).not.toHaveBeenCalled();
                expect(await harness.waitForKafkaMessage()).toBeUndefined();
            } finally {
                await harness.stop();
            }
        });

        it("rejects SOAP requests whose AXMLMessage root is not CCO", async () => {
            const harness = await createSoapServerHarness({ replicationEndpoint });
            const errorLogSpy = jest.spyOn(Config.logger, "error");

            try {
                await harness.sendSoapRequest(XmlEvents.UnrecognisedAXMLMessage);

                expect(errorLogSpy).toHaveBeenCalledWith(expect.stringContaining("Invalid request received"));
                expect(APMService.noticeError).toHaveBeenCalled();
                expect(await harness.waitForKafkaMessage()).toBeUndefined();
            } finally {
                await harness.stop();
            }
        });
    });

    describe("ReceiveMessage success path", () => {
        it("publishes valid train-update XML to Kafka", async () => {
            const harness = await createSoapServerHarness();

            const locationEventObject = {
                CCO: {
                    MensajeVidaSIE: false,
                    ActualizarDatosTren: {
                        trenPar: "6846",
                        fechaCreacion: "20-12-2022",
                        pasoTren: [{
                            haSalido: "true",
                            haEntrado: "true",
                            horaEntradaReal: "62520",
                            horaSalidaReal: "62522",
                        }],
                    },
                },
            };

            const expectedObject = `<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="3bedad6f-a633-4e5e-841f-80230c5b70cf" sequence="488534" xsi:type="CCO">
                                    <ActualizarDatosTren>
                                        <trenPar>6846</trenPar>
                                        <trenImpar>6846</trenImpar>
                                        <fechaCreacion>20-12-2022</fechaCreacion>
                                        <numeroRegistro>11581635</numeroRegistro>
                                        <operadorComercial>METROOP</operadorComercial>
                                        <pasoTren>
                                            <tipoCambio>2</tipoCambio>
                                            <estacion>3</estacion>
                                            <idPaso>210799231</idPaso>
                                            <horaEntrada>62520</horaEntrada>
                                            <horaEntradaReal>62520</horaEntradaReal>
                                            <haEntrado>true</haEntrado>
                                            <tipoParada>5</tipoParada>
                                            <paridad>p</paridad>
                                            <sentido>0</sentido>
                                            <horaSalida>-1</horaSalida>
                                            <horaSalidaReal>62522</horaSalidaReal>
                                            <haSalido>true</haSalido>
                                            <viaEntradaMallas>3</viaEntradaMallas>
                                            <retrasoEntrada>-21</retrasoEntrada>
                                            <viaCirculacionMallas>-</viaCirculacionMallas>
                                            <retrasoSalida>0</retrasoSalida>
                                            <horaInicioDetencion>-1</horaInicioDetencion>
                                            <duracionDetencion>-1</duracionDetencion>
                                        </pasoTren>
                                        <codigoOperadorComercial>-1</codigoOperadorComercial>
                                        <origenActualizaTren>GAC</origenActualizaTren>
                                    </ActualizarDatosTren>
                                </CCO>`;

            try {
                const response = await harness.sendSoapRequest(createReceiveMessageRequest(locationEventObject));
                const receivedMessage = await harness.waitForKafkaMessage();

                expect(response.status).toBe(200);
                expect(receivedMessage?.value.replaceAll(" ", "")).toStrictEqual(expectedObject.replaceAll(" ", ""));
                expect(APMService.incrementMetric).toHaveBeenCalledWith(
                    expect.stringContaining("message_counter"),
                    1,
                );
                expect(APMService.incrementMetric).toHaveBeenCalledWith(
                    expect.stringContaining("message_delay"),
                    expect.any(Number),
                );
            } finally {
                await harness.stop();
            }
        });

        it("accepts production-shaped multi-pasoTren CCO payloads", async () => {
            const harness = await createSoapServerHarness();

            try {
                const response = await harness.sendSoapRequest(wrapCcoInReceiveMessageSoap(SAMPLE_CCO_XML));
                const receivedMessage = await harness.waitForKafkaMessage();

                expect(response.status).toBe(200);
                expect(receivedMessage?.value.replace(/\s+/g, "")).toContain("<trenPar>V187</trenPar>");
                expect(receivedMessage?.value).toContain("<estacion>0</estacion>");
                expect(receivedMessage?.value).toContain("<estacion>800</estacion>");
            } finally {
                await harness.stop();
            }
        });

        it("does not record message_delay for MensajeVidaSIE heartbeat messages", async () => {
            const harness = await createSoapServerHarness();

            try {
                await harness.sendSoapRequest(wrapCcoInReceiveMessageSoap(HEARTBEAT_CCO_XML));
                const receivedMessage = await harness.waitForKafkaMessage();

                expect(receivedMessage?.value).toContain("<MensajeVidaSIE>true</MensajeVidaSIE>");
                expect(APMService.incrementMetric).not.toHaveBeenCalledWith(
                    expect.stringContaining("message_delay"),
                    expect.any(Number),
                );
            } finally {
                await harness.stop();
            }
        });

        it("logs Kafka publish failures but still completes the SOAP request", async () => {
            const failingProducer = new MockKafka(Config.logger, {}, {
                brokers: ["localhost:9092"],
                on: () => ({}),
            } as never);
            failingProducer.publish = jest.fn().mockRejectedValue(new Error("kafka unavailable"));

            const harness = await createSoapServerHarness({ kafkaProducer: failingProducer });
            const errorLogSpy = jest.spyOn(Config.logger, "error");

            try {
                const response = await harness.sendSoapRequest(wrapCcoInReceiveMessageSoap(SAMPLE_CCO_XML));
                const responseText = await response.text();

                expect(response.status).toBe(200);
                expect(responseText).toContain("OK");
                expect(failingProducer.publish).toHaveBeenCalled();
                expect(errorLogSpy).toHaveBeenCalledWith(expect.stringContaining("kafka unavailable"));
                expect(APMService.noticeError).toHaveBeenCalled();
            } finally {
                await harness.stop();
            }
        });
    });

    describe("Azure replication", () => {
        beforeEach(() => {
            jest.mocked(EventStore).mockClear();
        });

        it("replicates receivedMessage to EventStore when replication is enabled", async () => {
            const harness = await createSoapServerHarness({ replicationEndpoint });

            try {
                await harness.sendSoapRequest(wrapCcoInReceiveMessageSoap(SAMPLE_CCO_XML));
                const receivedMessage = await harness.waitForKafkaMessage();

                expect(mockEventStorePut).toHaveBeenCalledTimes(1);
                const replicatedBody = mockEventStorePut.mock.calls[0][0];
                expect(replicatedBody).toContain("<trenPar>V187</trenPar>");
                expect(receivedMessage?.value.replace(/\s+/g, "")).toBe(replicatedBody.replace(/\s+/g, ""));
            } finally {
                await harness.stop();
            }
        });

        it("constructs EventStore with provider train", async () => {
            const harness = await createSoapServerHarness({ replicationEndpoint });

            try {
                await harness.sendSoapRequest(wrapCcoInReceiveMessageSoap(SAMPLE_CCO_XML));

                expect(EventStore).toHaveBeenCalledWith(
                    Config.logger,
                    replicationEndpoint,
                    "train",
                );
            } finally {
                await harness.stop();
            }
        });

        it("skips replication when REPLICATION_ENDPOINT is unset", async () => {
            const harness = await createSoapServerHarness();

            try {
                await harness.sendSoapRequest(wrapCcoInReceiveMessageSoap(SAMPLE_CCO_XML));

                expect(mockEventStorePut).not.toHaveBeenCalled();
            } finally {
                await harness.stop();
            }
        });
    });

    describe("unsupported SOAP operations", () => {
        it("returns NOT_IMPLEMENTED for Login", async () => {
            const harness = await createSoapServerHarness();

            try {
                const response = await harness.sendSoapRequest(wrapLoginSoapRequest());
                const responseText = await response.text();

                expect(responseText).toContain("Method Not Implemented");
            } finally {
                await harness.stop();
            }
        });

        it("returns NOT_IMPLEMENTED for UploadMessages", async () => {
            const harness = await createSoapServerHarness();

            try {
                const response = await harness.sendSoapRequest(wrapUploadMessagesSoapRequest());
                const responseText = await response.text();

                expect(responseText).toContain("Method Not Implemented");
            } finally {
                await harness.stop();
            }
        });
    });
});
