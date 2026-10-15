import { GtfsApi } from "../../../src/services/gtfs-api";

jest.mock("../../../src/config", () => ({
    Config: {
        staticCCUrl: "https://test-static-api.com",
        axios: {
            get: jest.fn(),
        },
        logger: {
            warn: jest.fn(),
            error: jest.fn(),
        },
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

describe("GTFS API", () => {
    let gtfsApi: GtfsApi;
    let originalSetTimeout: typeof setTimeout;

    beforeEach(() => {
        jest.clearAllMocks();

        const { UtilsCommon } = require("realtime-common");

        // Default behavior: retry just executes the function
        (UtilsCommon.retry as jest.Mock).mockImplementation((fn) => fn());

        // Mock setTimeout to prevent actual scheduling
        originalSetTimeout = global.setTimeout;
        global.setTimeout = jest.fn() as any;

        gtfsApi = new GtfsApi();

        // Restore setTimeout
        global.setTimeout = originalSetTimeout;
    });

    it("should returned stops from the CC Static API", async () => {
        const { Config } = require("../../../src/config");

        const mockStopsData = [
            { stop_code: "123", stop_lat: -36.844, stop_lon: 174.769 },
            { stop_code: "456", stop_lat: -36.845, stop_lon: 174.770 },
        ];

        (Config.axios.get as jest.Mock).mockResolvedValue({
            data: mockStopsData,
        });

        await gtfsApi.fetchStops();

        expect(Config.axios.get).toHaveBeenCalledWith(
            "https://test-static-api.com/gtfs/stops?fields=stop_code,stop_lon,stop_lat"
        );

        // Verify stops are stored
        const stop1 = gtfsApi.getStopInfoByStopCode("123");
        expect(stop1).toEqual({ stop_code: "123", stop_lat: -36.844, stop_lon: 174.769 });

        const stop2 = gtfsApi.getStopInfoByStopCode("456");
        expect(stop2).toEqual({ stop_code: "456", stop_lat: -36.845, stop_lon: 174.770 });
    });

    it("should record an error when the call to CC Static API fails", async () => {
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

        await gtfsApi.fetchStops();

        expect(Config.axios.get).toHaveBeenCalledWith(
            "https://test-static-api.com/gtfs/stops?fields=stop_code,stop_lon,stop_lat"
        );
        expect(ApmCommon.APMService.noticeError).toHaveBeenCalledWith(mockError);
        expect(Config.logger.error).toHaveBeenCalledWith(
            expect.stringContaining("Could not fetch stops from")
        );
        expect(Config.logger.error).toHaveBeenCalledWith(
            expect.stringContaining("Network error")
        );
    });
});
