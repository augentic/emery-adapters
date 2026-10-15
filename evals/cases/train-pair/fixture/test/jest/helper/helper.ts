import { TrainEvent } from "../../../src/types/train-event";

export function createReceiveMessageRequest(event: TrainEvent): string {
    return `
    <?xml version="1.0"?>
    <soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:urn="urn:wsTrainIntf-IwsTrain" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
    <soapenv:Header>
        <SOAPAction>urn:wsTrainIntf-IwsTrain#ReceiveMessage</SOAPAction>
    </soapenv:Header>
    <soapenv:Body>
        <urn:ReceiveMessage>
            <AUserName>username</AUserName>
            <APassword>password</APassword>
            <AXMLMessage>
            <![CDATA[
                <CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="3bedad6f-a633-4e5e-841f-80230c5b70cf" sequence="488534" xsi:type="CCO">
                    <ActualizarDatosTren>
                        <trenPar>${event.CCO?.ActualizarDatosTren?.trenPar}</trenPar>
                        <trenImpar>6846</trenImpar>
                        <fechaCreacion>${event.CCO?.ActualizarDatosTren?.fechaCreacion}</fechaCreacion>
                        <numeroRegistro>11581635</numeroRegistro>
                        <operadorComercial>METROOP</operadorComercial>
                        <pasoTren>
                            <tipoCambio>2</tipoCambio>
                            <estacion>3</estacion>
                            <idPaso>210799231</idPaso>
                            <horaEntrada>62520</horaEntrada>
                            <horaEntradaReal>${event.CCO?.ActualizarDatosTren?.pasoTren?.[0]?.horaEntradaReal}</horaEntradaReal>
                            <haEntrado>${event.CCO?.ActualizarDatosTren?.pasoTren?.[0]?.haEntrado}</haEntrado>
                            <tipoParada>5</tipoParada>
                            <paridad>p</paridad>
                            <sentido>0</sentido>
                            <horaSalida>-1</horaSalida>
                            <horaSalidaReal>${event.CCO?.ActualizarDatosTren?.pasoTren?.[0]?.horaSalidaReal}</horaSalidaReal>
                            <haSalido>${event.CCO?.ActualizarDatosTren?.pasoTren?.[0]?.haSalido}</haSalido>
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
                </CCO>
            ]]>
            </AXMLMessage>
        </urn:ReceiveMessage>
    </soapenv:Body>
    </soapenv:Envelope>`;
}
