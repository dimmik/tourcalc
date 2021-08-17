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


export default class SpendingsDetail extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open,
            spendingInfo: props.spendingInfo,
            person: props.person
        }
    }
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
                                {this.state.spendingInfo.map((si, idx) => {
                                    return this.props.received ? (
                                        <TableRow key={idx} hover style={{
                                            backgroundColor:
                                                si.isSpendingToAll ? "white" :
                                                    (si.receivedAmountInCents == si.totalSpendingAmountInCents ? "LemonChiffon"
                                                        : "LightGray")
                                        }}>
                                        <TableCell component="th" scope="row">
                                            {si.from}
                                        </TableCell>
                                            <TableCell component="th" scope="row">
                                                {/*<!--pre>{JSON.stringify(si, null, 2)}</pre>*/}
                                                {si.receivedAmountInCents} ({(si.receivedAmountInCents * 100 / si.totalSpendingAmountInCents).toFixed(0)}% of {si.totalSpendingAmountInCents})
                                        </TableCell>
                                        <TableCell component="th" scope="row">
                                            {si.spendingDescription}
                                        </TableCell>
                                        </TableRow>
                                    ) :
                                        (
                                            <TableRow key={idx} hover>
                                                <TableCell component="th" scope="row">
                                                    {si.from}
                                                </TableCell>
                                                <TableCell component="th" scope="row">
                                                    {si.totalSpendingAmountInCents}
                                        </TableCell>
                                                <TableCell component="th" scope="row">
                                                    {si.spendingDescription}
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