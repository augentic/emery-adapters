import { IPropertyConverter, JsonConverter, JsonElementType, JsonObject, JsonProperty, JsonType } from "ta-json";

/**
 * Custom deserialiser for event type.
 */
class ChangeTypeConverter implements IPropertyConverter {
    public serialize(property: ChangeType): number {
        // need to force number type
        return Number(property.valueOf());
    }

    public deserialize(value: number): ChangeType {
        // need to force number type
        return Number(value);
    }
}

/**
 * Convert string to boolean.
 * Needed since all fields come as strings when deserialised from xml.
 * I thought ta-jason would be clever enough to check the type and do a conversion.
 */
class BooleanConverter implements IPropertyConverter {
    public serialize(property: boolean): string {
        return property ? "true" : "false";
    }

    public deserialize(value: string): boolean {
        return value === "true";
    }
}

/**
 * Convert string to number.
 * Needed since all fields come as strings when deserialised from xml.
 * I thought ta-jason would be clever enough to check the type and do a conversion.
 */
class NumberConverter implements IPropertyConverter {
    public serialize(property: number): string {
        return String(property);
    }

    public deserialize(value: string): number {
        return Number(value);
    }
}

export enum MovementType {
    ARRIVAL,
    DEPARTURE,
    PREDICTION,
    OTHER
}

export enum ChangeType {
    ExitedFirstStation = 1,
    ReachedFinalDestination = 2,
    ArrivedAtStation = 3,
    ExitedStation = 4,
    PassedStationWithoutStopping = 5,
    DetainedInPark = 6,
    /** Created by the operator. */
    DetainedAtStation = 7,
    StationNoLongerPartOfTheRun = 8,
    PlatformChange = 9,
    ExitLineChange = 10,
    ScheduleChange = 11
}

/**
 * Train update change.
 */
@JsonObject()
export class Change {
    @JsonProperty("tipoCambio")
    @JsonConverter(ChangeTypeConverter)
    public changeType: ChangeType;

    @JsonProperty("estacion")
    @JsonConverter(NumberConverter)
    public station: number;

    /** Unique id for the entry. */
    @JsonProperty("idPaso")
    public entryId: string;

    /**
     * Scheduled arrival time as per schedule.
     * In seconds from train update creation date at midnight.
     */
    @JsonProperty("horaEntrada")
    @JsonConverter(NumberConverter)
    public arrivalTime: number;

    /**
     * Actual arrival, or estimated arrival time (based on the latest actual arrival or departure time of the preceding stations).
     * In seconds from train update creation date at midnight.
     * -1 if not available.
     */
    @JsonProperty("horaEntradaReal")
    @JsonConverter(NumberConverter)
    public actualArrivalTime: number;

    @JsonProperty("haEntrado")
    @JsonConverter(BooleanConverter)
    public hasArrived: boolean;

    /**
     * Difference between the actual and scheduled arrival times if the train has already arrived at the station,
     * 0 otherwise.
     */
    @JsonProperty("retrasoEntrada")
    @JsonConverter(NumberConverter)
    public arrivalDelay: number;

    /**
     * Scheduled departure time as per schedule.
     * In seconds from train update creation date at midnight.
     */
    @JsonProperty("horaSalida")
    @JsonConverter(NumberConverter)
    public departureTime: number;

    /**
     * Actual departure, or estimated departure time (based on the latest actual arrival or departure time of the preceding stations).
     * In seconds from train update creation date at midnight.
     * -1 if not available.
     */
    @JsonProperty("horaSalidaReal")
    @JsonConverter(NumberConverter)
    public actualDepartureTime: number;

    @JsonProperty("haSalido")
    @JsonConverter(BooleanConverter)
    public hasDeparted: boolean;

    /**
     * Difference between the actual and scheduled arrival times if the train has already arrived at the station,
     * 0 otherwise.
     */
    @JsonProperty("retrasoSalida")
    @JsonConverter(NumberConverter)
    public departureDelay: number;

    @JsonProperty("horaInicioDetencion")
    @JsonConverter(NumberConverter)
    public detentionTime: number;

    @JsonProperty("duracionDetencion")
    @JsonConverter(NumberConverter)
    public detentionDuration: number;

    @JsonProperty("viaEntradaMallas")
    public platform: string;

    @JsonProperty("viaCirculacionMallas")
    public exitLine: string;


    /**
     * Train direction in reference to the platform?
     *  0 - right
     *  1 - left
     * -1 - unspecified
     */
    @JsonProperty("sentido")
    @JsonConverter(NumberConverter)
    public trainDirection: number;

    /**
     * Should be an enum, but again, we don't have the full list.
     * 4 - Original, Passing (non-stop/skip), or Destination (no dwell time in timetable)
     * 5 - Intermediate stop (there is a dwell time in the time table).
     */
    @JsonProperty("tipoParada")
    @JsonConverter(NumberConverter)
    public stopType: number;

    /** Not sure what this is used for. */
    @JsonProperty("paridad")
    public parity: string;
}

/**
 * Train update as received from the rail feed.
 * Defines the XML mappings as defined by the upstream provider - in Spanish.
 *
 * Used to deserialize ActualizarDatosTren elements.
 */
@JsonObject()
export class TrainUpdate {
    @JsonProperty("trenPar")
    public evenTrainId: string;

    @JsonProperty("trenImpar")
    public oddTrainId: string;

    @JsonType(String)
    @JsonProperty("fechaCreacion")
    public createdDate: string;

    @JsonProperty("numeroRegistro")
    public registrationNumber: string;

    /** This should really be an enum type with the following values FREIGHT, METRO, EXMETRO */
    @JsonProperty("operadorComercial")
    public trainType: string;

    @JsonProperty("codigoOperadorComercial")
    public trainTypeCode: string;

    @JsonProperty("trenCompleto")
    public fullTrain: string;

    @JsonProperty("origenActualizaTren")
    public trainUpdateSource: string;

    /**
     * NOTE: Only the first child will be used. The rest is a schedule only.
     * Date associated with the passage of the train through each station. Includes
     * one element for the station that the train has arrived at, plus one element
     * for each of the stations that the system has not detected the train arriving
     * at yet.
     */
    @JsonProperty("pasoTren")
    @JsonElementType(Change)
    public changes: Change[] = [];
}

@JsonObject()
export class Message {
    @JsonProperty("ActualizarDatosTren")
    public trainUpdate: TrainUpdate;
}
