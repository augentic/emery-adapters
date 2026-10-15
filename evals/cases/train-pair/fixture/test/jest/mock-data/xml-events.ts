export class XmlEvents {
    public static EmptyAXMLMessage = `
        <?xml version="1.0"?>
        <soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:urn="urn:wsTrainIntf-IwsTrain" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
        <soapenv:Header>
        <SOAPAction>urn:wsTrainIntf-IwsTrain#ReceiveMessage</SOAPAction>
        </soapenv:Header>
        <soapenv:Body>
        <urn:ReceiveMessage>
        <AUserName>username</AUserName>
        <APassword>password</APassword>
        </urn:ReceiveMessage>
        </soapenv:Body>
        </soapenv:Envelope>`;

    public static UnrecognisedAXMLMessage = `
        <?xml version="1.0"?>
        <soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:urn="urn:wsTrainIntf-IwsTrain" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
        <soapenv:Header>
        <SOAPAction>urn:wsTrainIntf-IwsTrain#ReceiveMessage</SOAPAction>
        </soapenv:Header>
        <soapenv:Body>
        <urn:ReceiveMessage>
        <AUserName>username</AUserName>
        <APassword>password</APassword>
        <AXMLMessage><![CDATA[<UnexpectedRoot><data>not a CCO message</data></UnexpectedRoot>]]></AXMLMessage>
        </urn:ReceiveMessage>
        </soapenv:Body>
        </soapenv:Envelope>`;
}
