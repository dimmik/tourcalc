import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

export default class TourAdd extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open
        }
    }

    name = ""
    accessCode = "باحوس"

    render() {
        return (
            <span>
                <span onClick={() => this.setState({ dialogOpen: true })}>
                    {this.props.children}
                </span>
            <Dialog aria-labelledby="customized-dialog-title" open={this.state.dialogOpen}>
                <DialogTitle id="customized-dialog-title">Add Tour</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => {
                            event.preventDefault();
                            }}>
                            <p>name:</p>
                            <input
                                type='text'
                                onChange={(e) => this.name = event.target.value}
                                defaultValue={this.name}
                            />
                            {this.props.chooseCode ? (
                                <span><p>Access Code:</p>
                                <input
                                    type='text'
                                    onChange={(e) => this.accessCode = event.target.value}
                                    defaultValue={this.accessCode}
                                /></span>) : <span />
                            }
                        </form>
                </DialogContent>
                <DialogActions>
                        <button color="primary" onClick={() => {
                                AppState.addTour(this.props.app, this.name, this.accessCode)
                                .then(this.setState({ dialogOpen: false }))
                                .then(() => { AppState.loadTours(this.props.app, this.props.tourid) })

                        }}>{this.props.actionButtonText}</button>
                        <button onClick={() => { this.setState({ dialogOpen: false }) }}>Cancel</button>
                </DialogActions>
            </Dialog>
            </span>
        )
    }
}
