import * as http from "http";
import * as express from "express";

import { Application } from "express";
export class Server {
    private app: Application;
    private server: http.Server;
    public stops: { stop_code: string, stop_lat: number, stop_lon: number }[] = [];
    public blockMgt: any = {};

    public async start(port: number) {
        this.app = express();

        this.app.get("/gtfs/stops", (req, res) => {
            res.send(this.stops);
        });

        this.app.get("/allocations/trips/", (req, res) => {
            res.send(this.blockMgt);
        });

        await new Promise<void>((res) => {
            this.server = this.app.listen(port, res);
        });
    }

    public async stop() {
        await new Promise((res) => {
            this.server.close(res);
        });
    }

    public async setStops(stops: { stop_code: string, stop_lat: number, stop_lon: number }[]) {
        this.stops = stops;
    }

    public async setBlockMgtApi(blockMgtResponse: any) {
        this.blockMgt = blockMgtResponse;
    }
}
