export interface IRaftMetrics {
    id: number,
    current_term: number,
    vote: IVote,
    last_log_index?: number,
    last_applied?: ILogId,
    snapshot?: ILogId,
    purged?: ILogId,
    state?: ServerState,
    current_leader: number,
    millis_since_quorum_ack?: number,
    membership_config: IStoredMembership,
    replication: Map<number, ILogId>
}

export interface IVote {
    leader_id: ILeaderId,
    committed: boolean,
}

export interface ILeaderId {
    term: number,
    node_id: number,
}

export interface ILogId {
    leader_id: ILeaderId,
    index: number,
}

export enum ServerState {
    Learner,
    Follower,
    Candidate,
    Leader,
    Shutdown,
}

export interface IStoredMembership {
    log_id?: ILogId,
    membership: IMembership,
}

export interface IMembership {
    configs: number[],
    nodes: Map<number, INode>
}

export interface INode {
    id: number,
    addr_raft: string,
    addr_api: string,
}