export interface TrainEvent {
    CCO?: {
        MensajeVidaSIE: boolean;
        ActualizarDatosTren?: {
            trenPar?: string;
            fechaCreacion?: string;
            pasoTren?: { haSalido: string; haEntrado: string; horaEntradaReal: string; horaSalidaReal: string }[];
        };
    };
}
