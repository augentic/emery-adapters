export const SAMPLE_CCO_XML = `<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="3bedad6f-a633-4e5e-841f-80230c5b70cf" sequence="488534" xsi:type="CCO">
    <ActualizarDatosTren>
        <trenPar>V187</trenPar>
        <trenImpar>V187</trenImpar>
        <fechaCreacion>13/06/2026</fechaCreacion>
        <numeroRegistro>12962066</numeroRegistro>
        <operadorComercial>METRO</operadorComercial>
        <pasoTren>
            <tipoCambio>3</tipoCambio>
            <estacion>0</estacion>
            <idPaso>229102216</idPaso>
            <horaEntradaReal>48580</horaEntradaReal>
            <haEntrado>true</haEntrado>
            <haSalido>false</haSalido>
        </pasoTren>
        <pasoTren>
            <tipoCambio>11</tipoCambio>
            <estacion>800</estacion>
            <idPaso>229102217</idPaso>
            <haEntrado>false</haEntrado>
            <haSalido>false</haSalido>
        </pasoTren>
        <origenActualizaTren>GAC</origenActualizaTren>
    </ActualizarDatosTren>
</CCO>`;

export function wrapCcoInReceiveMessageSoap(ccoXml: string): string {
    return `<?xml version="1.0"?>
<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:urn="urn:wsTrainIntf-IwsTrain" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <soapenv:Header>
    <SOAPAction>urn:wsTrainIntf-IwsTrain#ReceiveMessage</SOAPAction>
  </soapenv:Header>
  <soapenv:Body>
    <urn:ReceiveMessage>
      <AUserName>username</AUserName>
      <APassword>password</APassword>
      <AXMLMessage><![CDATA[${ccoXml}]]></AXMLMessage>
    </urn:ReceiveMessage>
  </soapenv:Body>
</soapenv:Envelope>`;
}

export const HEARTBEAT_CCO_XML = `<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="heartbeat" sequence="1" xsi:type="CCO">
    <MensajeVidaSIE>true</MensajeVidaSIE>
</CCO>`;

export function wrapLoginSoapRequest(): string {
    return `<?xml version="1.0"?>
<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:urn="urn:wsTrainIntf-IwsTrain" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <soapenv:Header>
    <SOAPAction>urn:wsTrainIntf-IwsTrain#Login</SOAPAction>
  </soapenv:Header>
  <soapenv:Body>
    <urn:Login>
      <s_UserName>username</s_UserName>
      <s_Password>password</s_Password>
    </urn:Login>
  </soapenv:Body>
</soapenv:Envelope>`;
}

export function wrapUploadMessagesSoapRequest(): string {
    return `<?xml version="1.0"?>
<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:urn="urn:wsTrainIntf-IwsTrain" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <soapenv:Header>
    <SOAPAction>urn:wsTrainIntf-IwsTrain#UploadMessages</SOAPAction>
  </soapenv:Header>
  <soapenv:Body>
    <urn:UploadMessages>
      <s_XMLMessages><![CDATA[<CCO></CCO>]]></s_XMLMessages>
    </urn:UploadMessages>
  </soapenv:Body>
</soapenv:Envelope>`;
}
