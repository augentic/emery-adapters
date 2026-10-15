import * as moment from "moment-timezone";
import { Main } from "../../../src/main";
import { Server } from "../support/server";
import { assert } from "chai";
import { Config } from "../../../src/config";
import { after, before, binding, given, then, when } from "cucumber-tsflow";
import { ApmCommon, ExpressCommon, KafkaCommon } from "realtime-common";

const { APMService } = ApmCommon;

@binding()
class MainSteps {
    private main: Main;
    private server: Server;
    private producer: KafkaCommon.KafkaProducer;
    private consumer: KafkaCommon.KafkaConsumer;
    private result: any;
    private stationId: any;

    @before()
    public async before() {
        Config.staticCCUrl = "http://localhost:3001";
        Config.blockMgtUrl = "http://localhost:3001";

        Config.getAzureAccessTokenRetrieverConfig = () => {
            return {
                clientId: "",
                domain: "transit.example.com",
                keyVault: {
                    host: "",
                    secretNameSystemClientSecret: "system-client-secret",
                },
                loggerConfig: {
                    logger: Config.logger as never,
                },
                localDevEnv: {
                    accessToken: "supersecrettoken",
                },
            };
        };

        this.consumer = new KafkaCommon.MockKafka(
            Config.logger,
            Config.kafka.consumer.endpoints,
            [Config.kafka.producer.vpTopic],
            "any"
        );

        this.consumer.onMessage((message) => {
            this.result = message;
        });

        this.producer = new KafkaCommon.MockKafka(
            Config.logger,
            Config.kafka.consumer.endpoints,
            Config.kafka.consumer.topics[0],
        );

        await APMService.initialize(Config);
        const webApp = new ExpressCommon.ExpressWebAppWrapper(Config.logger, { customErrorHandler: true });
        await webApp.start(Config.port)
            .then(() => Config.logger.info("Status endpoint is running"));

        this.main = new Main(
            webApp,
            new KafkaCommon.MockKafka(
                Config.logger,
                Config.kafka.consumer.endpoints,
                Config.kafka.consumer.topics,
                Config.kafka.consumer.consumerGroup
            ),
            new KafkaCommon.MockKafka(
                Config.logger,
                Config.kafka.producer.endpoints,
                Config.kafka.producer.vpTopic
            )
        );


        this.server = new Server();
        this.server.setStops([
            {
                stop_code: "133",
                stop_lat: 33.33,
                stop_lon: -140.55,
            },
            {
                stop_code: "134",
                stop_lat: 34.213,
                stop_lon: -141.24,
            },
        ]);
        await this.server.start(3001);
        await this.main.start();
    }

    @after()
    public async after() {
        await this.main.stop();
        await this.server.stop();
    }

    @given(/^static stops information:$/)
    public async setStaticStopInfo(table: any) {
        this.server.setStops(table.hashes());
    }

    @given(/^vehicles for the trip "(.*)"$/)
    public setVehicles(vehicles: string): void {
        const vehicleLabels = vehicles.split(",");
        this.server.setBlockMgtApi({ all: vehicleLabels.map((label) => ({ vehicleLabel: label })) });
    }

    @given(/^filter for stations "(.*)"$/)
    public setFilterForStations(stations: string): void {
        Config.filterStations = stations.split(",");
    }

    @when(/^an event without (.*)$/)
    public receiveInvalidMessage(invaliType: string): void {
        let event = "";
        if(invaliType === "trainUpdate") {
            event = `<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="7c104b58-25cb-437a-8c39-297633a6638e" sequence="1214699" xsi:type="CCO">
                    </CCO>`;
        } else if(invaliType === "changes") {
            event = `<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="7c104b58-25cb-437a-8c39-297633a6638e" sequence="1214699" xsi:type="CCO">
            <ActualizarDatosTren>
                <trenPar>5226</trenPar>
                <trenImpar>5226</trenImpar>
            </ActualizarDatosTren>
            </CCO>`;
        } else if(invaliType === "actual changes") {
            event = `<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="7c104b58-25cb-437a-8c39-297633a6638e" sequence="1214699" xsi:type="CCO">
            <ActualizarDatosTren>
                <trenPar>5226</trenPar>
                <trenImpar>5226</trenImpar>
                <pasoTren>
                    <tipoCambio>3</tipoCambio>
                    <estacion>80</estacion>
                    <idPaso>181353261</idPaso>
                    <horaEntrada>-1</horaEntrada>
                    <horaEntradaReal>-1</horaEntradaReal>
                    <haEntrado>false</haEntrado>
                    <tipoParada>4</tipoParada>
                    <paridad>p</paridad>
                    <sentido>0</sentido>
                    <horaSalida>58080</horaSalida>
                    <horaSalidaReal>58080</horaSalidaReal>
                    <haSalido>false</haSalido>
                    <viaEntradaMallas>2</viaEntradaMallas>
                    <retrasoEntrada>-3</retrasoEntrada>
                    <viaCirculacionMallas>2</viaCirculacionMallas>
                    <retrasoSalida>0</retrasoSalida>
                    <horaInicioDetencion>-1</horaInicioDetencion>
                    <duracionDetencion>-1</duracionDetencion>
                </pasoTren>
            </ActualizarDatosTren>
            </CCO>`;
        }
        this.producer.publish({ value: event });
    }

    @when(/^an (.*) event for "(.*)" is received with "(.*)" seconds delay$/)
    public receiveMessage(eventType: string, stationId: string, delayInSeconds: string): void {

        const currentTime = moment.tz(Config.timezone);
        const date = currentTime.format(Config.dateFormat);
        const seconds = currentTime.diff(currentTime.clone().startOf("day"), "seconds") - Number(delayInSeconds);
        this.stationId = stationId;

        const event = `
        <CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="7c104b58-25cb-437a-8c39-297633a6638e" sequence="1214699" xsi:type="CCO">
        <ActualizarDatosTren>
            <trenPar>5226</trenPar>
            <trenImpar>5226</trenImpar>
            <fechaCreacion>${date}</fechaCreacion>
            <numeroRegistro>9299669</numeroRegistro>
            <operadorComercial>METRO</operadorComercial>
            <pasoTren>
                <tipoCambio>${eventType === "arrival" ? 3 : 1}</tipoCambio>
                <estacion>${this.stationId}</estacion>
                <idPaso>181353261</idPaso>
                <horaEntrada>${seconds}</horaEntrada>
                <horaEntradaReal>${seconds}</horaEntradaReal>
                <haEntrado>${eventType === "arrival" ? "true" : "false"}</haEntrado>
                <tipoParada>4</tipoParada>
                <paridad>p</paridad>
                <sentido>0</sentido>
                <horaSalida>${seconds}</horaSalida>
                <horaSalidaReal>${seconds}</horaSalidaReal>
                <haSalido>${eventType === "arrival" ? "false" : "true"}</haSalido>
                <viaEntradaMallas>2</viaEntradaMallas>
                <retrasoEntrada>-3</retrasoEntrada>
                <viaCirculacionMallas>2</viaCirculacionMallas>
                <retrasoSalida>0</retrasoSalida>
                <horaInicioDetencion>-1</horaInicioDetencion>
                <duracionDetencion>-1</duracionDetencion>
            </pasoTren>
            <pasoTren>
                <tipoCambio>3</tipoCambio>
                <estacion>${this.stationId}</estacion>
                <idPaso>181353261</idPaso>
                <horaEntrada>58020</horaEntrada>
                <horaEntradaReal>58017</horaEntradaReal>
                <haEntrado>true</haEntrado>
                <tipoParada>4</tipoParada>
                <paridad>p</paridad>
                <sentido>0</sentido>
                <horaSalida>58080</horaSalida>
                <horaSalidaReal>58080</horaSalidaReal>
                <haSalido>false</haSalido>
                <viaEntradaMallas>2</viaEntradaMallas>
                <retrasoEntrada>-3</retrasoEntrada>
                <viaCirculacionMallas>2</viaCirculacionMallas>
                <retrasoSalida>0</retrasoSalida>
                <horaInicioDetencion>-1</horaInicioDetencion>
                <duracionDetencion>-1</duracionDetencion>
            </pasoTren>
            <codigoOperadorComercial>-1</codigoOperadorComercial>
            <origenActualizaTren>GAC</origenActualizaTren>
        </ActualizarDatosTren></CCO>`;

        this.producer.publish({ value: event });
    }

    @then(/^(.*) (.*) vehicle events is created$/, undefined, 15000)
    public async receiveVehicleEvent(numberOfEvents: string, eventType: string) {
        let eventNumber = 0;
        await new Promise<void>((resolve) => {
            const interval = setInterval(() => {
                if (this.result) {
                    const vehicleEvent = JSON.parse(this.result.value);

                    assert(vehicleEvent.eventType === "Location");
                    assert(vehicleEvent.remoteData.externalId === this.server.blockMgt.all[0].vehicleLabel);
                    assert(vehicleEvent.locationData.gpsAccuracy === 0);

                    const stopCode = Config.STATION_ID_TO_STOP_CODE_MAP[this.stationId];
                    let stop = Config.DEPARTURE_LOCATION_OVERWRITE[parseInt(stopCode, 10)];

                    if (eventType === "arrival") {
                        stop = this.server.stops.find((data) => data.stop_code === stopCode)!;
                    }

                    assert(vehicleEvent.locationData.latitude === stop.stop_lat);
                    assert(vehicleEvent.locationData.longitude === stop.stop_lon);

                    if (numberOfEvents === "2" && eventNumber === 0) {
                        eventNumber++;
                        this.result = null;
                    } else {
                        clearInterval(interval);
                        resolve();
                    }
                }
            }, 100);
        });
    }

    @then(/^no event should be generated$/)
    public async noEvent() {
        await new Promise<void>((res) => {
            setTimeout(() => {
                if (!this.result) {
                    res();
                }
            }, 1000);
        });
    }
}
