import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableRow from '@material-ui/core/TableRow';
import Paper from '@material-ui/core/Paper';

import Button from '@material-ui/core/Button';

import Chart from "react-google-charts";



export default class SpendingsDetail extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open,
            spendingInfo: props.spendingInfo,
            person: props.person
        }
        //alert("props received: " + props.received);
        var alertX = true;
        if (props.received) {
            props.spendingInfo.forEach(
                si => {
                    if (si.type != null && si.type != "") {
                        if (this.summary[si.type] == null) this.summary[si.type] = 0;
                        this.summary[si.type] += si.receivedAmountInCents;
                        this.total += si.receivedAmountInCents;
                    }
                }
            );
        }
    }
    summary = {}
    total = 0;
    /*{
        from: Person 3,
        receivedAmountInCents: 250,
        totalSpendingAmountInCents: 1000,
        spendingDescription: 1000 to all,
        isSpendingToAll: true,
        toNames: []
    }*/

    render() {
        return (
            <span>
                <span onClick={() => this.setState({ dialogOpen: true, spendingInfo: this.props.spendingInfo })} style={{ cursor: "pointer" }}>
                    {this.props.showText}
                </span>
                <Dialog aria-labelledby="customized-dialog-title" open={this.state.dialogOpen} onClose={() => { this.setState({ dialogOpen: false }) }}>
                    <DialogTitle id="customized-dialog-title">{this.props.received ?  'Received' : 'Spent' } for {this.state.person.name}</DialogTitle>
                    <DialogContent>

                        <Table>
                            <TableBody>
                                {this.props.received ? (
                                <TableRow key={-1} hover>
                                    <TableCell component="th" scope="row" colSpan={2}>
                                            <b>SUMMARY:</b><br/>
                                            {Object.keys(this.summary).sort(
                                                (k1, k2) => {
                                                    return -(this.summary[k1] > this.summary[k2] ? 1 : (this.summary[k1] < this.summary[k2] ? -1 : 0))
                                                }
                                            ).map(key => (
                                                <span key={"summary-" + key}><nobr>{key}: <b>{this.summary[key]}</b> ({(this.summary[key] * 100 / this.total).toFixed(0)}%)</nobr><br /></span>
                                                ))}

                                    </TableCell>
                                        <TableCell component="th" scope="row" colSpan={1}>
                                            <Chart
                                                width={'300px'}
                                                height={'200px'}
                                                chartType="PieChart"
                                                loader={<div>Loading Chart</div>}
                                                options={{
                                                    chartArea: { width: '100%', height: '80%' },
                                                }}
                                                data={
                                                    [['Category', 'Amount']]
                                                        .concat(
                                                            Object.keys(this.summary).sort(
                                                                (k1, k2) => {
                                                                    return -(this.summary[k1] > this.summary[k2] ? 1 : (this.summary[k1] < this.summary[k2] ? -1 : 0))
                                                                }
                                                            ).map(key => [key, this.summary[key]])
                                                            
                                                        )

                                                }
                                                rootProps={{ 'data-testid': '1' }}
                                            />
                                    </TableCell>
                                    </TableRow>
                                ) 
                                    : (<TableRow key={-1} hover>
                                        <TableCell component="th" scope="row" colSpan={3}>
                                            &nbsp;
                                        </TableCell>
                                    </TableRow>) 
                                        }
                                {this.state.spendingInfo.map((si, idx) => {
                                    return this.props.received ? (
                                        <TableRow key={idx} hover style={{
                                            backgroundColor:
                                                si.isSpendingToAll ? "white" :
                                                    (si.toNames.length == 1 ? "LemonChiffon"
                                                        : "LightGray")
                                        }}>
                                        <TableCell component="th" scope="row">
                                            {si.from}
                                        </TableCell>
                                            <TableCell component="th" scope="row">
                                                {/*<!--pre>{JSON.stringify(si, null, 2)}</pre>*/}
                                                {si.receivedAmountInCents} <span style={{ fontSize: "xx-small" }}>({(si.receivedAmountInCents * 100 / si.totalSpendingAmountInCents).toFixed(0)}% of {si.totalSpendingAmountInCents})</span>
                                        </TableCell>
                                            <TableCell component="th" scope="row">
                                                (<span style={{ fontSize: "xx-small" }}><b><i>{si.type}</i></b></span>) {si.spendingDescription} 
                                        </TableCell>
                                        </TableRow>
                                    ) :
                                        (
                                            <TableRow key={idx} hover style={{
                                                backgroundColor:
                                                    si.isSpendingToAll ? "white" :
                                                        (si.toNames.length == 1 ? "LemonChiffon"
                                                            : "LightGray")
                                            }}>
                                                <TableCell component="th" scope="row">
                                                    {si.from}
                                                </TableCell>
                                                <TableCell component="th" scope="row">
                                                    {si.totalSpendingAmountInCents} <span style={{ fontSize: 'xx-small' }}>({si.isSpendingToAll ? "all" : (si.toNames.length == 1 ? "pers" : "part")})</span>
                                        </TableCell>
                                                <TableCell component="th" scope="row">
                                                    (<span style={{ fontSize: "xx-small" }}><b><i>{si.type}</i></b></span>) {si.spendingDescription}
                                                </TableCell>
                                            </TableRow>

                                            )
                                }

                                )}
                            </TableBody>
                        </Table>
                    </DialogContent>
                    <DialogActions>
                        <Button color='primary' variant='outlined' onClick={() => { this.setState({ dialogOpen: false }) }}>Ok, got it</Button>
                    </DialogActions>
                </Dialog>
            </span>
        )
    }
}