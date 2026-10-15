import { BlockMgtApi } from "../../../src/services/block-mgt-api";

jest.mock("../../../src/config", () => ({
    Config: {
        blockMgtUrl: "https://test-block-mgt-api.com",
        axios: {
            get: jest.fn(),
        },
        logger: {
            warn: jest.fn(),
            error: jest.fn(),
        },
        getAzureAccessTokenRetrieverConfig: jest.fn(),
    },
}));

jest.mock("realtime-common", () => ({
    ApmCommon: {
        APMService: {
            noticeError: jest.fn(),
        },
    },
    UtilsCommon: {
        retry: jest.fn(),
    },
    AxiosCommon: {},
}));

jest.mock("auth-common", () => ({
    AzureSystemAccessTokenRetriever: {
        provide: jest.fn(),
    },
}));

describe("Block Management API", () => {
    let blockMgtApi: BlockMgtApi;
    let mockAuthTokenRetriever: { retrieve: jest.Mock };

    beforeEach(async () => {
        jest.clearAllMocks();

        const { UtilsCommon } = require("realtime-common");
        const { AzureSystemAccessTokenRetriever } = require("auth-common");

        // Default behavior: retry just executes the function
        (UtilsCommon.retry as jest.Mock).mockImplementation((fn) => fn());

        // Mock auth token retriever
        mockAuthTokenRetriever = {
            retrieve: jest.fn().mockResolvedValue("mock-auth-token"),
        };
        (AzureSystemAccessTokenRetriever.provide as jest.Mock).mockResolvedValue(mockAuthTokenRetriever);

        blockMgtApi = new BlockMgtApi();
        await blockMgtApi.fetchAuthToken();
    });

    it("should return vehicle labels from the Block Management API", async () => {
        const { Config } = require("../../../src/config");

        const mockAllocationData = {
            all: [
                { vehicleLabel: "TRAIN-001" },
                { vehicleLabel: "TRAIN-002" },
                { vehicleLabel: "TRAIN-003" },
            ],
        };

        (Config.axios.get as jest.Mock).mockResolvedValue({
            data: mockAllocationData,
        });

        const externalRefId = "REF123";
        const result = await blockMgtApi.getVehiclesByExternalRefId(externalRefId);

        expect(Config.axios.get).toHaveBeenCalledWith(
            `https://test-block-mgt-api.com/allocations/trips?externalRefId=${externalRefId}&closestTrip=true&include_filtered_routes=true`,
            {
                headers: {
                    Authorization: "Bearer mock-auth-token",
                },
            }
        );

        expect(result).toEqual(["TRAIN-001", "TRAIN-002", "TRAIN-003"]);
        expect(mockAuthTokenRetriever.retrieve).toHaveBeenCalled();
    });

    it("should record an error when the call to Block Management API fails", async () => {
        const { Config } = require("../../../src/config");
        const { ApmCommon, UtilsCommon } = require("realtime-common");

        const mockError = new Error("Network error");
        mockError.stack = "Error: Network error\n  at test";

        (Config.axios.get as jest.Mock).mockRejectedValue(mockError);

        // Mock retry to execute function and catch/throw error
        (UtilsCommon.retry as jest.Mock).mockImplementation(async (fn, options) => {
            try {
                return await fn();
            } catch (err) {
                if (options && options.onRetry) {
                    // Call onRetry but don't actually retry
                }
                throw err;
            }
        });

        const externalRefId = "REF123";
        const result = await blockMgtApi.getVehiclesByExternalRefId(externalRefId);

        expect(Config.axios.get).toHaveBeenCalledWith(
            `https://test-block-mgt-api.com/allocations/trips?externalRefId=${externalRefId}&closestTrip=true&include_filtered_routes=true`,
            {
                headers: {
                    Authorization: "Bearer mock-auth-token",
                },
            }
        );
        expect(ApmCommon.APMService.noticeError).toHaveBeenCalledWith(mockError);
        expect(Config.logger.error).toHaveBeenCalledWith(
            expect.stringContaining("Could not fetch allocation from")
        );
        expect(Config.logger.error).toHaveBeenCalledWith(
            expect.stringContaining("Network error")
        );
        expect(result).toEqual([]);
    });
});
