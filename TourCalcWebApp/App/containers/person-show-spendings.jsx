import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';


export default class SpendingsDetail extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open
        }
        this.spendingInfo = props.spendingInfo
        this.person = props.person
    }
    spendingInfo = []
    person = null
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
                <span onClick={() => this.setState({ dialogOpen: true })} style={{ cursor: "pointer" }}>
                    {this.props.showText}
                </span>
                <Dialog aria-labelledby="customized-dialog-title" open={this.state.dialogOpen}>
                    <DialogTitle id="customized-dialog-title">Spendings for {this.person.name}</DialogTitle>
                    <DialogContent>
                        {this.spendingInfo.map((si, idx) => {
                            
                            return this.props.received ?(
                                <div key={idx}>
                                    {si.from} -- received {si.receivedAmountInCents} -- out of {si.totalSpendingAmountInCents}: {si.spendingDescription}
                                </div>

                                )
                                : (
                                    <div key={idx}>
                                        {si.from} -- spent {si.totalSpendingAmountInCents} -- : {si.spendingDescription}
                                    </div>

                                    )
                            }

                        )}
                    </DialogContent>
                    <DialogActions>
                        <button onClick={() => { this.setState({ dialogOpen: false }) }}>Ok, got it</button>
                    </DialogActions>
                </Dialog>
            </span>
        )
    }
}