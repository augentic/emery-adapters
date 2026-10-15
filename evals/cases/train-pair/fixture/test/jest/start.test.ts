import { ConfluentSecretRetriever } from "../../src/confluent-secret-retriever";
import * as start from "../../src/start";
import { Main } from "../../src/main";
import { FastifyAdapter } from "realtime-common/server";

jest.mock("../../src/main");
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

describe("Test start", () => {
    const MockedMain = jest.mocked(Main);
    let webApp: FastifyAdapter;
    beforeEach(async () => {
        ConfluentSecretRetriever.confluentKafkaSecret = { username: "", password: "" };
        MockedMain.mockImplementation((inputWebApp: FastifyAdapter) => {
            webApp = inputWebApp;
            return {
                start: jest.fn(),
            } as unknown as Main;
        });
    });

    afterEach(async () => {
        jest.clearAllMocks();
        webApp.close();
    });

    it("Should call start function", async () => {
        // need this to trigger the startup
        const whatever = (start as unknown as { main: never }).main;
        await new Promise((resolve) => setTimeout(resolve, 10));
        expect(MockedMain).toBeCalledTimes(1);
    });
});
